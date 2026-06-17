use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use base64::{engine::general_purpose, Engine as _};

use crate::performer::handler::{
    KittyFileTransfer, KittyFileTransferAction, KittyFileTransferCompression,
    KittyFileTransferFileType, KittyFileTransferTransmission,
};

const MAX_ACTIVE_SESSIONS: usize = 1;
const MAX_WRITE_FILES: usize = 4096;
const MAX_WRITE_FILE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_WRITE_SESSION_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_RECEIVE_REQUESTS: usize = 64;
const MAX_RECEIVE_FILES: usize = 4096;
const MAX_RECEIVE_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_RECEIVE_SESSION_BYTES: u64 = 256 * 1024 * 1024;
const MAX_RECEIVE_DEPTH: usize = 16;
const READ_CHUNK_BYTES: usize = 4096;
const STAGING_DIR: &str = ".staging";

#[derive(Debug, Default)]
pub(super) struct KittyFileTransferManager {
    sessions: HashMap<String, KittyFileTransferSession>,
}

#[derive(Debug, Default)]
pub(super) struct KittyFileTransferResponse {
    pub replies: Vec<String>,
    pub approval_request: Option<KittyFileTransferApprovalRequest>,
}

#[derive(Debug)]
pub(super) struct KittyFileTransferApprovalRequest {
    pub id: String,
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KittyFileTransferApproval {
    Pending,
    Approved,
}

#[derive(Debug)]
struct KittyFileTransferSession {
    id: String,
    terminator: String,
    approval: KittyFileTransferApproval,
    kind: KittyFileTransferSessionKind,
}

#[derive(Debug)]
enum KittyFileTransferSessionKind {
    Send(SendSession),
    Receive(ReceiveSession),
}

#[derive(Debug)]
struct SendSession {
    destination_root: PathBuf,
    final_root: PathBuf,
    staging_root: PathBuf,
    files: HashMap<String, WriteFileState>,
    total_written: u64,
    errored: bool,
}

#[derive(Debug)]
struct WriteFileState {
    kind: FileKind,
    compression: KittyFileTransferCompression,
    expected_size: Option<u64>,
    written: u64,
    compressed_written: u64,
    compressed_data: Vec<u8>,
    file: Option<File>,
    errored: bool,
}

#[derive(Debug)]
struct ReceiveSession {
    expected_paths: usize,
    requested_paths: Vec<ReceiveRequest>,
    entries: HashMap<String, ReceiveEntry>,
    total_listed_bytes: u64,
}

#[derive(Debug, Clone)]
struct ReceiveRequest {
    client_file_id: String,
    path: PathBuf,
}

#[derive(Debug, Clone)]
struct ReceiveEntry {
    actual_file_id: String,
    path: PathBuf,
    kind: FileKind,
    size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileKind {
    Regular,
    Directory,
}

impl KittyFileTransferResponse {
    fn status(
        id: &str,
        file_id: Option<&str>,
        status: &str,
        size: Option<u64>,
        name: Option<&str>,
        terminator: &str,
    ) -> Self {
        let mut response = Self::default();
        response.push_status(id, file_id, status, size, name, terminator);
        response
    }

    fn push_status(
        &mut self,
        id: &str,
        file_id: Option<&str>,
        status: &str,
        size: Option<u64>,
        name: Option<&str>,
        terminator: &str,
    ) {
        self.replies
            .push(status_reply(id, file_id, status, size, name, terminator));
    }
}

impl KittyFileTransferManager {
    pub(super) fn handle_approval(
        &mut self,
        id: &str,
        approved: bool,
    ) -> KittyFileTransferResponse {
        if !approved {
            let Some(session) = self.sessions.remove(id) else {
                return KittyFileTransferResponse::default();
            };
            cleanup_session(&session);
            return KittyFileTransferResponse::status(
                &session.id,
                None,
                "EPERM:User refused the transfer",
                None,
                None,
                &session.terminator,
            );
        }

        match self.sessions.get(id).map(|session| &session.kind) {
            Some(KittyFileTransferSessionKind::Send(_)) => self.approve_send(id),
            Some(KittyFileTransferSessionKind::Receive(_)) => self.approve_receive(id),
            None => KittyFileTransferResponse::default(),
        }
    }

    pub(super) fn handle_transfer(
        &mut self,
        transfer: KittyFileTransfer,
        terminator: &str,
    ) -> KittyFileTransferResponse {
        match transfer.action {
            KittyFileTransferAction::Send => self.start_send(transfer, terminator),
            KittyFileTransferAction::Receive => self.start_receive(transfer, terminator),
            KittyFileTransferAction::Cancel => self.cancel(transfer, terminator),
            KittyFileTransferAction::File => self.handle_file(transfer, terminator),
            KittyFileTransferAction::Data => {
                self.handle_data(transfer, terminator, false)
            }
            KittyFileTransferAction::EndData => {
                self.handle_data(transfer, terminator, true)
            }
            KittyFileTransferAction::Status => KittyFileTransferResponse::status(
                &transfer.id,
                transfer.file_id.as_deref(),
                "ENOSYS:Client status commands are not implemented",
                None,
                None,
                terminator,
            ),
            KittyFileTransferAction::Finish => self.finish(transfer, terminator),
        }
    }

    fn start_send(
        &mut self,
        transfer: KittyFileTransfer,
        terminator: &str,
    ) -> KittyFileTransferResponse {
        if unsupported_transmission(&transfer) {
            return unsupported_transmission_response(&transfer, terminator);
        }
        if let Some(response) = self.reject_duplicate_or_busy(&transfer.id, terminator) {
            return response;
        }

        let destination_root = match default_destination_root() {
            Ok(root) => root,
            Err(status) => {
                return KittyFileTransferResponse::status(
                    &transfer.id,
                    None,
                    status,
                    None,
                    None,
                    terminator,
                );
            }
        };
        let segment = local_session_segment(&transfer.id);
        let final_root = destination_root.join(&segment);
        let staging_root = destination_root.join(STAGING_DIR).join(&segment);

        self.sessions.insert(
            transfer.id.clone(),
            KittyFileTransferSession {
                id: transfer.id.clone(),
                terminator: terminator.to_owned(),
                approval: KittyFileTransferApproval::Pending,
                kind: KittyFileTransferSessionKind::Send(SendSession {
                    destination_root: destination_root.clone(),
                    final_root,
                    staging_root,
                    files: HashMap::new(),
                    total_written: 0,
                    errored: false,
                }),
            },
        );

        KittyFileTransferResponse {
            replies: Vec::new(),
            approval_request: Some(KittyFileTransferApprovalRequest {
                id: transfer.id,
                title: "File transfer request".to_string(),
                body: format!(
                    "A terminal program wants to write files into {}",
                    destination_root.display()
                ),
            }),
        }
    }

    fn start_receive(
        &mut self,
        transfer: KittyFileTransfer,
        terminator: &str,
    ) -> KittyFileTransferResponse {
        if unsupported_transmission(&transfer) {
            return unsupported_transmission_response(&transfer, terminator);
        }
        if let Some(response) = self.reject_duplicate_or_busy(&transfer.id, terminator) {
            return response;
        }
        let Some(expected_paths) = transfer.size else {
            return KittyFileTransferResponse::status(
                &transfer.id,
                None,
                "EINVAL:Receive session is missing path count",
                None,
                None,
                terminator,
            );
        };
        let Ok(expected_paths) = usize::try_from(expected_paths) else {
            return KittyFileTransferResponse::status(
                &transfer.id,
                None,
                "EFBIG:Too many requested paths",
                None,
                None,
                terminator,
            );
        };
        if expected_paths == 0 || expected_paths > MAX_RECEIVE_REQUESTS {
            return KittyFileTransferResponse::status(
                &transfer.id,
                None,
                "EFBIG:Too many requested paths",
                None,
                None,
                terminator,
            );
        }

        self.sessions.insert(
            transfer.id.clone(),
            KittyFileTransferSession {
                id: transfer.id,
                terminator: terminator.to_owned(),
                approval: KittyFileTransferApproval::Pending,
                kind: KittyFileTransferSessionKind::Receive(ReceiveSession {
                    expected_paths,
                    requested_paths: Vec::new(),
                    entries: HashMap::new(),
                    total_listed_bytes: 0,
                }),
            },
        );
        KittyFileTransferResponse::default()
    }

    fn reject_duplicate_or_busy(
        &self,
        id: &str,
        terminator: &str,
    ) -> Option<KittyFileTransferResponse> {
        if self.sessions.contains_key(id) {
            return Some(KittyFileTransferResponse::status(
                id,
                None,
                "EEXIST:File transfer session id is already active",
                None,
                None,
                terminator,
            ));
        }
        if self.sessions.len() >= MAX_ACTIVE_SESSIONS {
            return Some(KittyFileTransferResponse::status(
                id,
                None,
                "EBUSY:Another file transfer session is active",
                None,
                None,
                terminator,
            ));
        }
        None
    }

    fn approve_send(&mut self, id: &str) -> KittyFileTransferResponse {
        let result = match self.sessions.get_mut(id) {
            Some(session) if session.approval == KittyFileTransferApproval::Pending => {
                match &mut session.kind {
                    KittyFileTransferSessionKind::Send(send) => {
                        prepare_send_session(send).map(|()| {
                            session.approval = KittyFileTransferApproval::Approved;
                        })
                    }
                    KittyFileTransferSessionKind::Receive(_) => {
                        Err("EPERM:No pending file transfer session")
                    }
                }
            }
            _ => Err("EPERM:No pending file transfer session"),
        };

        match result {
            Ok(()) => {
                let Some(session) = self.sessions.get(id) else {
                    return KittyFileTransferResponse::default();
                };
                KittyFileTransferResponse::status(
                    &session.id,
                    None,
                    "OK",
                    None,
                    None,
                    &session.terminator,
                )
            }
            Err(status) => {
                let Some(session) = self.sessions.remove(id) else {
                    return KittyFileTransferResponse::default();
                };
                cleanup_session(&session);
                KittyFileTransferResponse::status(
                    &session.id,
                    None,
                    status,
                    None,
                    None,
                    &session.terminator,
                )
            }
        }
    }

    fn approve_receive(&mut self, id: &str) -> KittyFileTransferResponse {
        let Some(session) = self.sessions.get(id) else {
            return KittyFileTransferResponse::default();
        };
        let KittyFileTransferSessionKind::Receive(receive) = &session.kind else {
            return KittyFileTransferResponse::default();
        };
        let session_id = session.id.clone();
        let terminator = session.terminator.clone();
        let requests = receive.requested_paths.clone();

        let mut response = KittyFileTransferResponse::default();
        response.push_status(&session_id, None, "OK", None, None, &terminator);

        let mut entries = HashMap::new();
        let mut next_id = 1;
        let mut total_bytes = 0;
        for request in &requests {
            if let Err(status) = list_receive_request(
                &session_id,
                request,
                &mut entries,
                &mut response,
                &mut next_id,
                &mut total_bytes,
                &terminator,
            ) {
                response.push_status(
                    &session_id,
                    Some(&request.client_file_id),
                    status,
                    None,
                    None,
                    &terminator,
                );
            }
        }

        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_default();
        response.push_status(&session_id, None, "OK", None, Some(&home), &terminator);

        let Some(session) = self.sessions.get_mut(id) else {
            return response;
        };
        if let KittyFileTransferSessionKind::Receive(receive) = &mut session.kind {
            receive.entries = entries;
            receive.total_listed_bytes = total_bytes;
        }
        session.approval = KittyFileTransferApproval::Approved;
        response
    }

    fn handle_file(
        &mut self,
        transfer: KittyFileTransfer,
        terminator: &str,
    ) -> KittyFileTransferResponse {
        match self.sessions.get(&transfer.id).map(|session| &session.kind) {
            Some(KittyFileTransferSessionKind::Receive(_)) => {
                self.handle_receive_file(transfer, terminator)
            }
            _ => self.handle_send_file(transfer, terminator),
        }
    }

    fn handle_send_file(
        &mut self,
        transfer: KittyFileTransfer,
        terminator: &str,
    ) -> KittyFileTransferResponse {
        let file_id = transfer.file_id.clone();
        let status = self.try_handle_send_file(&transfer);
        KittyFileTransferResponse::status(
            &transfer.id,
            file_id.as_deref(),
            status,
            None,
            None,
            terminator,
        )
    }

    fn try_handle_send_file(&mut self, transfer: &KittyFileTransfer) -> &'static str {
        if unsupported_transmission(transfer) {
            return "ENOSYS:Rsync file transfers are not implemented";
        }
        let Some(file_id) = &transfer.file_id else {
            return "EINVAL:Missing file id";
        };
        let Some(name) = &transfer.name else {
            return "EINVAL:Missing file name";
        };
        if self.drop_pending_send_session(&transfer.id) {
            return "EPERM:File transfer command arrived before approval";
        }

        let session = match self.send_session_mut(&transfer.id) {
            Ok(session) => session,
            Err(status) => return status,
        };
        if session.files.len() >= MAX_WRITE_FILES {
            session.errored = true;
            return "EFBIG:Too many files in transfer session";
        }
        if session.files.contains_key(file_id) {
            session.errored = true;
            return "EEXIST:File id already exists";
        }
        if matches!(
            transfer.file_type,
            KittyFileTransferFileType::Symlink | KittyFileTransferFileType::Link
        ) {
            session.errored = true;
            return "ENOSYS:Links are not supported";
        }
        if transfer.size.unwrap_or(0) > MAX_WRITE_FILE_BYTES {
            session.errored = true;
            return "EFBIG:File is too large";
        }

        let path = match destination_path(&session.staging_root, name) {
            Ok(path) => path,
            Err(status) => {
                session.errored = true;
                return status;
            }
        };
        let Some(parent) = path.parent() else {
            session.errored = true;
            return "EINVAL:Invalid file transfer path";
        };

        match transfer.file_type {
            KittyFileTransferFileType::Directory => {
                if transfer.compression != KittyFileTransferCompression::None {
                    session.errored = true;
                    return "EINVAL:Compression is only supported for regular files";
                }
                if fs::create_dir_all(parent).is_err()
                    || fs::create_dir_all(&path).is_err()
                {
                    session.errored = true;
                    return "EIO:Could not create directory";
                }
                session.files.insert(
                    file_id.clone(),
                    WriteFileState {
                        kind: FileKind::Directory,
                        compression: KittyFileTransferCompression::None,
                        expected_size: transfer.size,
                        written: 0,
                        compressed_written: 0,
                        compressed_data: Vec::new(),
                        file: None,
                        errored: false,
                    },
                );
                "OK"
            }
            KittyFileTransferFileType::Regular => {
                if fs::create_dir_all(parent).is_err() {
                    session.errored = true;
                    return "EIO:Could not create parent directory";
                }
                let file =
                    match OpenOptions::new().write(true).create_new(true).open(&path) {
                        Ok(file) => file,
                        Err(_) => {
                            session.errored = true;
                            return "EEXIST:Destination file already exists";
                        }
                    };
                session.files.insert(
                    file_id.clone(),
                    WriteFileState {
                        kind: FileKind::Regular,
                        compression: transfer.compression,
                        expected_size: transfer.size,
                        written: 0,
                        compressed_written: 0,
                        compressed_data: Vec::new(),
                        file: Some(file),
                        errored: false,
                    },
                );
                "STARTED"
            }
            KittyFileTransferFileType::Symlink | KittyFileTransferFileType::Link => {
                unreachable!("links were rejected before opening files")
            }
        }
    }

    fn handle_receive_file(
        &mut self,
        transfer: KittyFileTransfer,
        terminator: &str,
    ) -> KittyFileTransferResponse {
        let approval = self
            .sessions
            .get(&transfer.id)
            .map(|session| session.approval);
        match approval {
            Some(KittyFileTransferApproval::Pending) => {
                self.collect_receive_request(transfer, terminator)
            }
            Some(KittyFileTransferApproval::Approved) => {
                self.send_receive_file_data(transfer, terminator)
            }
            None => KittyFileTransferResponse::status(
                &transfer.id,
                transfer.file_id.as_deref(),
                "EPERM:No approved file transfer session",
                None,
                None,
                terminator,
            ),
        }
    }

    fn collect_receive_request(
        &mut self,
        transfer: KittyFileTransfer,
        terminator: &str,
    ) -> KittyFileTransferResponse {
        if unsupported_transmission(&transfer) {
            return unsupported_transmission_response(&transfer, terminator);
        }
        let Some(file_id) = &transfer.file_id else {
            return KittyFileTransferResponse::status(
                &transfer.id,
                None,
                "EINVAL:Missing file id",
                None,
                None,
                terminator,
            );
        };
        let Some(name) = &transfer.name else {
            return KittyFileTransferResponse::status(
                &transfer.id,
                transfer.file_id.as_deref(),
                "EINVAL:Missing file name",
                None,
                None,
                terminator,
            );
        };
        let path = match receive_path(name) {
            Ok(path) => path,
            Err(status) => {
                return KittyFileTransferResponse::status(
                    &transfer.id,
                    Some(file_id),
                    status,
                    None,
                    None,
                    terminator,
                );
            }
        };

        let Some(session) = self.sessions.get_mut(&transfer.id) else {
            return KittyFileTransferResponse::status(
                &transfer.id,
                Some(file_id),
                "EPERM:No approved file transfer session",
                None,
                None,
                terminator,
            );
        };
        let KittyFileTransferSessionKind::Receive(receive) = &mut session.kind else {
            return KittyFileTransferResponse::status(
                &transfer.id,
                Some(file_id),
                "EPERM:No approved file transfer session",
                None,
                None,
                terminator,
            );
        };
        if receive.requested_paths.len() >= receive.expected_paths {
            self.sessions.remove(&transfer.id);
            return KittyFileTransferResponse::status(
                &transfer.id,
                Some(file_id),
                "EINVAL:Too many receive paths",
                None,
                None,
                terminator,
            );
        }
        receive.requested_paths.push(ReceiveRequest {
            client_file_id: file_id.clone(),
            path,
        });
        if receive.requested_paths.len() < receive.expected_paths {
            return KittyFileTransferResponse::default();
        }

        KittyFileTransferResponse {
            replies: Vec::new(),
            approval_request: Some(KittyFileTransferApprovalRequest {
                id: transfer.id,
                title: "File read request".to_string(),
                body: receive_preview(receive),
            }),
        }
    }

    fn send_receive_file_data(
        &mut self,
        transfer: KittyFileTransfer,
        terminator: &str,
    ) -> KittyFileTransferResponse {
        if unsupported_transmission(&transfer) {
            return unsupported_transmission_response(&transfer, terminator);
        }
        let Some(reply_file_id) = transfer.file_id.as_deref() else {
            return KittyFileTransferResponse::status(
                &transfer.id,
                None,
                "EINVAL:Missing file id",
                None,
                None,
                terminator,
            );
        };
        let Some(entry) = self.receive_entry_for_request(&transfer) else {
            return KittyFileTransferResponse::status(
                &transfer.id,
                Some(reply_file_id),
                "EPERM:File was not listed for this receive session",
                None,
                None,
                terminator,
            );
        };
        if entry.kind != FileKind::Regular {
            return KittyFileTransferResponse::status(
                &transfer.id,
                Some(reply_file_id),
                "EISDIR:Cannot stream directory data",
                None,
                None,
                terminator,
            );
        }
        if entry.size > MAX_RECEIVE_FILE_BYTES {
            return KittyFileTransferResponse::status(
                &transfer.id,
                Some(reply_file_id),
                "EFBIG:File is too large",
                None,
                None,
                terminator,
            );
        }
        match safe_symlink_metadata(&entry.path) {
            Ok(metadata) if metadata.is_file() => {}
            Ok(_) => {
                return KittyFileTransferResponse::status(
                    &transfer.id,
                    Some(reply_file_id),
                    "ENOSYS:Special files are not supported",
                    None,
                    None,
                    terminator,
                );
            }
            Err(status) => {
                return KittyFileTransferResponse::status(
                    &transfer.id,
                    Some(reply_file_id),
                    status,
                    None,
                    None,
                    terminator,
                );
            }
        }

        if transfer.compression == KittyFileTransferCompression::Zlib {
            return self.send_compressed_receive_file_data(
                &transfer,
                reply_file_id,
                &entry,
                terminator,
            );
        }

        let mut file = match File::open(&entry.path) {
            Ok(file) => file,
            Err(_) => {
                return KittyFileTransferResponse::status(
                    &transfer.id,
                    Some(reply_file_id),
                    "EIO:Could not read",
                    None,
                    None,
                    terminator,
                );
            }
        };
        let mut response = KittyFileTransferResponse::default();
        let mut previous: Option<Vec<u8>> = None;
        let mut buffer = [0; READ_CHUNK_BYTES];
        loop {
            let count = match file.read(&mut buffer) {
                Ok(count) => count,
                Err(_) => {
                    response.push_status(
                        &transfer.id,
                        Some(reply_file_id),
                        "EIO:Could not read",
                        None,
                        None,
                        terminator,
                    );
                    return response;
                }
            };
            if count == 0 {
                let final_chunk = previous.take().unwrap_or_default();
                response.replies.push(data_reply(
                    &transfer.id,
                    reply_file_id,
                    KittyFileTransferAction::EndData,
                    &final_chunk,
                    terminator,
                ));
                return response;
            }
            if let Some(chunk) = previous.replace(buffer[..count].to_vec()) {
                response.replies.push(data_reply(
                    &transfer.id,
                    reply_file_id,
                    KittyFileTransferAction::Data,
                    &chunk,
                    terminator,
                ));
            }
        }
    }

    fn send_compressed_receive_file_data(
        &mut self,
        transfer: &KittyFileTransfer,
        reply_file_id: &str,
        entry: &ReceiveEntry,
        terminator: &str,
    ) -> KittyFileTransferResponse {
        let mut file = match File::open(&entry.path) {
            Ok(file) => file,
            Err(_) => {
                return KittyFileTransferResponse::status(
                    &transfer.id,
                    Some(reply_file_id),
                    "EIO:Could not read",
                    None,
                    None,
                    terminator,
                );
            }
        };
        let mut raw = Vec::new();
        if Read::by_ref(&mut file)
            .take(MAX_RECEIVE_FILE_BYTES.saturating_add(1))
            .read_to_end(&mut raw)
            .is_err()
        {
            return KittyFileTransferResponse::status(
                &transfer.id,
                Some(reply_file_id),
                "EIO:Could not read",
                None,
                None,
                terminator,
            );
        }
        if raw.len() as u64 > MAX_RECEIVE_FILE_BYTES {
            return KittyFileTransferResponse::status(
                &transfer.id,
                Some(reply_file_id),
                "EFBIG:File is too large",
                None,
                None,
                terminator,
            );
        }
        let compressed = match zlib_compress(&raw) {
            Ok(compressed) => compressed,
            Err(status) => {
                return KittyFileTransferResponse::status(
                    &transfer.id,
                    Some(reply_file_id),
                    status,
                    None,
                    None,
                    terminator,
                );
            }
        };
        let mut response = KittyFileTransferResponse::default();
        push_data_replies(
            &mut response,
            &transfer.id,
            reply_file_id,
            &compressed,
            terminator,
        );
        response
    }

    fn receive_entry_for_request(
        &self,
        transfer: &KittyFileTransfer,
    ) -> Option<ReceiveEntry> {
        let session = self.sessions.get(&transfer.id)?;
        let KittyFileTransferSessionKind::Receive(receive) = &session.kind else {
            return None;
        };
        if let Some(name) = &transfer.name {
            if let Ok(path) = receive_path(name) {
                let key = path_key(&path)?;
                if let Some(entry) = receive.entries.get(&key) {
                    return Some(entry.clone());
                }
            }
        }
        let file_id = transfer.file_id.as_deref()?;
        receive
            .entries
            .values()
            .find(|entry| entry.actual_file_id == file_id)
            .cloned()
    }

    fn handle_data(
        &mut self,
        transfer: KittyFileTransfer,
        terminator: &str,
        finish_file: bool,
    ) -> KittyFileTransferResponse {
        if matches!(
            self.sessions.get(&transfer.id).map(|session| &session.kind),
            Some(KittyFileTransferSessionKind::Receive(_))
        ) {
            return KittyFileTransferResponse::status(
                &transfer.id,
                transfer.file_id.as_deref(),
                "ENOSYS:Rsync receive data is not implemented",
                None,
                None,
                terminator,
            );
        }

        let file_id = transfer.file_id.clone();
        let (status, size) = self.try_handle_send_data(&transfer, finish_file);
        KittyFileTransferResponse::status(
            &transfer.id,
            file_id.as_deref(),
            status,
            size,
            None,
            terminator,
        )
    }

    fn try_handle_send_data(
        &mut self,
        transfer: &KittyFileTransfer,
        finish_file: bool,
    ) -> (&'static str, Option<u64>) {
        let Some(file_id) = &transfer.file_id else {
            return ("EINVAL:Missing file id", None);
        };
        if unsupported_transmission(transfer) {
            return ("ENOSYS:Rsync file transfers are not implemented", None);
        }
        if self.drop_pending_send_session(&transfer.id) {
            return ("EPERM:File transfer command arrived before approval", None);
        }
        let session = match self.send_session_mut(&transfer.id) {
            Ok(session) => session,
            Err(status) => return (status, None),
        };
        let Some(file) = session.files.get_mut(file_id) else {
            return ("EPERM:File was not started", None);
        };
        if file.errored {
            return ("EIO:File transfer already failed", Some(file.written));
        }
        if transfer.compression != KittyFileTransferCompression::None
            && transfer.compression != file.compression
        {
            file.errored = true;
            session.errored = true;
            return (
                "EINVAL:Compression changed during file transfer",
                Some(file.written),
            );
        }
        if file.kind != FileKind::Regular {
            file.errored = true;
            session.errored = true;
            return (
                "EINVAL:Cannot write data to a directory",
                Some(file.written),
            );
        }

        if file.compression == KittyFileTransferCompression::Zlib {
            if let Some(data) = &transfer.data {
                let chunk_len = data.len() as u64;
                let new_compressed_size =
                    match file.compressed_written.checked_add(chunk_len) {
                        Some(size) => size,
                        None => {
                            file.errored = true;
                            session.errored = true;
                            return (
                                "EFBIG:Compressed file data is too large",
                                Some(file.written),
                            );
                        }
                    };
                if new_compressed_size > MAX_WRITE_FILE_BYTES {
                    file.errored = true;
                    session.errored = true;
                    return (
                        "EFBIG:Compressed file data is too large",
                        Some(file.written),
                    );
                }
                file.compressed_data.extend_from_slice(data);
                file.compressed_written = new_compressed_size;
            }

            if !finish_file {
                return ("PROGRESS", Some(file.compressed_written));
            }

            return finish_compressed_send_file(
                file,
                &mut session.total_written,
                &mut session.errored,
            );
        }

        if let Some(data) = &transfer.data {
            let chunk_len = data.len() as u64;
            let new_file_size = match file.written.checked_add(chunk_len) {
                Some(size) => size,
                None => {
                    file.errored = true;
                    session.errored = true;
                    return ("EFBIG:File is too large", Some(file.written));
                }
            };
            let new_session_size = match session.total_written.checked_add(chunk_len) {
                Some(size) => size,
                None => {
                    file.errored = true;
                    session.errored = true;
                    return ("EFBIG:Transfer session is too large", Some(file.written));
                }
            };
            if new_file_size > MAX_WRITE_FILE_BYTES
                || file
                    .expected_size
                    .is_some_and(|expected| new_file_size > expected)
            {
                file.errored = true;
                session.errored = true;
                return ("EFBIG:File is too large", Some(file.written));
            }
            if new_session_size > MAX_WRITE_SESSION_BYTES {
                file.errored = true;
                session.errored = true;
                return ("EFBIG:Transfer session is too large", Some(file.written));
            }
            let Some(handle) = file.file.as_mut() else {
                file.errored = true;
                session.errored = true;
                return ("EIO:File is not open", Some(file.written));
            };
            if handle.write_all(data).is_err() {
                file.errored = true;
                session.errored = true;
                file.file = None;
                return ("EIO:Failed to write file data", Some(file.written));
            }
            file.written = new_file_size;
            session.total_written = new_session_size;
        }

        if !finish_file {
            return ("PROGRESS", Some(file.written));
        }

        file.file = None;
        if let Some(expected) = file.expected_size {
            if file.written != expected {
                file.errored = true;
                session.errored = true;
                return ("EIO:File size mismatch", Some(file.written));
            }
        }
        ("OK", Some(file.written))
    }

    fn finish(
        &mut self,
        transfer: KittyFileTransfer,
        terminator: &str,
    ) -> KittyFileTransferResponse {
        let Some(mut session) = self.sessions.remove(&transfer.id) else {
            return KittyFileTransferResponse::status(
                &transfer.id,
                transfer.file_id.as_deref(),
                "EPERM:No approved file transfer session",
                None,
                None,
                terminator,
            );
        };

        match &mut session.kind {
            KittyFileTransferSessionKind::Receive(_) => {
                KittyFileTransferResponse::status(
                    &session.id,
                    None,
                    "OK",
                    None,
                    None,
                    terminator,
                )
            }
            KittyFileTransferSessionKind::Send(send) => {
                let status = if session.approval != KittyFileTransferApproval::Approved {
                    "EPERM:File transfer session is waiting for approval"
                } else if send.errored || send.files.values().any(|file| file.errored) {
                    "EIO:Transfer session has errors"
                } else if send.files.values().any(|file| file.file.is_some()) {
                    "EIO:Transfer contains unfinished files"
                } else if send.final_root.exists() {
                    "EEXIST:Destination already exists"
                } else {
                    for file in send.files.values_mut() {
                        file.file = None;
                    }
                    match fs::rename(&send.staging_root, &send.final_root) {
                        Ok(()) => "OK",
                        Err(_) => "EIO:Could not commit transfer",
                    }
                };

                let total_written = send.total_written;
                let staging_root = send.staging_root.clone();
                if status != "OK" {
                    let _ = fs::remove_dir_all(staging_root);
                }
                KittyFileTransferResponse::status(
                    &session.id,
                    None,
                    status,
                    Some(total_written),
                    None,
                    terminator,
                )
            }
        }
    }

    fn cancel(
        &mut self,
        transfer: KittyFileTransfer,
        terminator: &str,
    ) -> KittyFileTransferResponse {
        if let Some(session) = self.sessions.remove(&transfer.id) {
            cleanup_session(&session);
        }
        KittyFileTransferResponse::status(
            &transfer.id,
            transfer.file_id.as_deref(),
            "CANCELED",
            None,
            None,
            terminator,
        )
    }

    fn send_session_mut(&mut self, id: &str) -> Result<&mut SendSession, &'static str> {
        let session = self
            .sessions
            .get_mut(id)
            .ok_or("EPERM:No approved file transfer session")?;
        if session.approval != KittyFileTransferApproval::Approved {
            return Err("EPERM:File transfer session is waiting for approval");
        }
        match &mut session.kind {
            KittyFileTransferSessionKind::Send(send) => Ok(send),
            KittyFileTransferSessionKind::Receive(_) => {
                Err("EPERM:No approved file transfer session")
            }
        }
    }

    fn drop_pending_send_session(&mut self, id: &str) -> bool {
        if !matches!(
            self.sessions.get(id),
            Some(KittyFileTransferSession {
                approval: KittyFileTransferApproval::Pending,
                kind: KittyFileTransferSessionKind::Send(_),
                ..
            })
        ) {
            return false;
        }
        if let Some(session) = self.sessions.remove(id) {
            cleanup_session(&session);
        }
        true
    }

    #[cfg(test)]
    pub(super) fn active_session_count(&self) -> usize {
        self.sessions.len()
    }

    #[cfg(test)]
    pub(super) fn set_session_destination_root(
        &mut self,
        id: &str,
        destination_root: PathBuf,
    ) {
        let segment = local_session_segment(id);
        let session = self
            .sessions
            .get_mut(id)
            .expect("file transfer session should exist");
        let KittyFileTransferSessionKind::Send(send) = &mut session.kind else {
            panic!("file transfer session should be a send session");
        };
        send.destination_root = destination_root.clone();
        send.final_root = destination_root.join(&segment);
        send.staging_root = destination_root.join(STAGING_DIR).join(segment);
    }
}

fn unsupported_transmission(transfer: &KittyFileTransfer) -> bool {
    transfer.transmission != KittyFileTransferTransmission::Simple
}

fn unsupported_transmission_response(
    transfer: &KittyFileTransfer,
    terminator: &str,
) -> KittyFileTransferResponse {
    KittyFileTransferResponse::status(
        &transfer.id,
        transfer.file_id.as_deref(),
        "ENOSYS:Rsync file transfers are not implemented",
        None,
        None,
        terminator,
    )
}

fn status_reply(
    id: &str,
    file_id: Option<&str>,
    status: &str,
    size: Option<u64>,
    name: Option<&str>,
    terminator: &str,
) -> String {
    let encoded_status = general_purpose::STANDARD.encode(status.as_bytes());
    let mut reply = format!("\x1b]5113;ac=status;id={id};");
    if let Some(file_id) = file_id {
        reply.push_str(&format!("fid={file_id};"));
    }
    reply.push_str(&format!("st={encoded_status};"));
    if let Some(size) = size {
        reply.push_str(&format!("sz={size};"));
    }
    if let Some(name) = name {
        reply.push_str(&format!("n={};", general_purpose::STANDARD.encode(name)));
    }
    reply.push_str(terminator);
    reply
}

fn file_metadata_reply(
    session_id: &str,
    client_file_id: &str,
    entry: &ReceiveEntry,
    parent: Option<&str>,
    terminator: &str,
) -> String {
    let name = entry.path.to_string_lossy();
    let file_type = match entry.kind {
        FileKind::Regular => "regular",
        FileKind::Directory => "directory",
    };
    let mut reply = format!(
        "\x1b]5113;ac=file;id={session_id};fid={client_file_id};st={};n={};ft={file_type};sz={};",
        general_purpose::STANDARD.encode(entry.actual_file_id.as_bytes()),
        general_purpose::STANDARD.encode(name.as_bytes()),
        entry.size
    );
    if let Some(parent) = parent {
        reply.push_str(&format!("parent={parent};"));
    }
    reply.push_str(terminator);
    reply
}

fn data_reply(
    session_id: &str,
    file_id: &str,
    action: KittyFileTransferAction,
    data: &[u8],
    terminator: &str,
) -> String {
    let action = match action {
        KittyFileTransferAction::Data => "data",
        KittyFileTransferAction::EndData => "end_data",
        _ => unreachable!("only data actions can transfer file contents"),
    };
    format!(
        "\x1b]5113;ac={action};id={session_id};fid={file_id};d={}{}",
        general_purpose::STANDARD.encode(data),
        terminator
    )
}

fn push_data_replies(
    response: &mut KittyFileTransferResponse,
    session_id: &str,
    file_id: &str,
    data: &[u8],
    terminator: &str,
) {
    if data.is_empty() {
        response.replies.push(data_reply(
            session_id,
            file_id,
            KittyFileTransferAction::EndData,
            data,
            terminator,
        ));
        return;
    }

    let mut chunks = data.chunks(READ_CHUNK_BYTES).peekable();
    while let Some(chunk) = chunks.next() {
        let action = if chunks.peek().is_none() {
            KittyFileTransferAction::EndData
        } else {
            KittyFileTransferAction::Data
        };
        response
            .replies
            .push(data_reply(session_id, file_id, action, chunk, terminator));
    }
}

fn finish_compressed_send_file(
    file: &mut WriteFileState,
    session_total_written: &mut u64,
    session_errored: &mut bool,
) -> (&'static str, Option<u64>) {
    let decompressed = match zlib_decompress(&file.compressed_data, MAX_WRITE_FILE_BYTES)
    {
        Ok(decompressed) => decompressed,
        Err(status) => {
            file.errored = true;
            *session_errored = true;
            return (status, Some(file.written));
        }
    };
    let decompressed_len = decompressed.len() as u64;
    if file
        .expected_size
        .is_some_and(|expected| decompressed_len != expected)
    {
        file.errored = true;
        *session_errored = true;
        return ("EIO:File size mismatch", Some(decompressed_len));
    }
    let new_session_size = match session_total_written.checked_add(decompressed_len) {
        Some(size) => size,
        None => {
            file.errored = true;
            *session_errored = true;
            return ("EFBIG:Transfer session is too large", Some(file.written));
        }
    };
    if new_session_size > MAX_WRITE_SESSION_BYTES {
        file.errored = true;
        *session_errored = true;
        return ("EFBIG:Transfer session is too large", Some(file.written));
    }
    let Some(handle) = file.file.as_mut() else {
        file.errored = true;
        *session_errored = true;
        return ("EIO:File is not open", Some(file.written));
    };
    if handle.write_all(&decompressed).is_err() {
        file.errored = true;
        *session_errored = true;
        file.file = None;
        return ("EIO:Failed to write file data", Some(file.written));
    }
    file.file = None;
    file.written = decompressed_len;
    *session_total_written = new_session_size;
    ("OK", Some(file.written))
}

fn zlib_decompress(data: &[u8], max_size: u64) -> Result<Vec<u8>, &'static str> {
    let mut decompressed = Vec::new();
    flate2::read::ZlibDecoder::new(data)
        .take(max_size.saturating_add(1))
        .read_to_end(&mut decompressed)
        .map_err(|_| "EINVAL:Could not decompress file data")?;
    if decompressed.len() as u64 > max_size {
        return Err("EFBIG:File is too large");
    }
    Ok(decompressed)
}

fn zlib_compress(data: &[u8]) -> Result<Vec<u8>, &'static str> {
    let mut encoder =
        flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder
        .write_all(data)
        .map_err(|_| "EIO:Could not compress file data")?;
    encoder
        .finish()
        .map_err(|_| "EIO:Could not compress file data")
}

fn default_destination_root() -> Result<PathBuf, &'static str> {
    if let Some(root) = std::env::var_os("XDG_DOWNLOAD_DIR") {
        return Ok(PathBuf::from(root).join("mars-transfers"));
    }
    if let Some(home) =
        std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))
    {
        return Ok(PathBuf::from(home).join("Downloads").join("mars-transfers"));
    }
    Err("EIO:No home directory for file transfer destination")
}

fn local_session_segment(id: &str) -> String {
    let mut segment = String::from("session-");
    for byte in id.as_bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'.' | b'-' => {
                segment.push(*byte as char);
            }
            _ => segment.push_str(&format!("_{byte:02x}")),
        }
    }
    segment
}

fn receive_preview(receive: &ReceiveSession) -> String {
    let mut body = format!(
        "A terminal program wants to read {} local path(s):",
        receive.requested_paths.len()
    );
    for request in receive.requested_paths.iter().take(6) {
        body.push('\n');
        body.push_str(&request.path.to_string_lossy());
    }
    if receive.requested_paths.len() > 6 {
        body.push_str("\n...");
    }
    body
}

fn list_receive_request(
    session_id: &str,
    request: &ReceiveRequest,
    entries: &mut HashMap<String, ReceiveEntry>,
    response: &mut KittyFileTransferResponse,
    next_id: &mut u64,
    total_bytes: &mut u64,
    terminator: &str,
) -> Result<(), &'static str> {
    let metadata = safe_symlink_metadata(&request.path)?;
    if metadata.is_file() {
        push_receive_entry(
            session_id,
            &request.client_file_id,
            None,
            &request.path,
            FileKind::Regular,
            metadata.len(),
            entries,
            response,
            next_id,
            total_bytes,
            terminator,
        )
        .map(|_| ())
    } else if metadata.is_dir() {
        push_receive_entry(
            session_id,
            &request.client_file_id,
            None,
            &request.path,
            FileKind::Directory,
            0,
            entries,
            response,
            next_id,
            total_bytes,
            terminator,
        )?;
        let parent_id = entries
            .get(&path_key(&request.path).ok_or("EINVAL:Invalid receive path")?)
            .map(|entry| entry.actual_file_id.clone())
            .ok_or("EIO:Could not list directory")?;
        list_directory(
            session_id,
            &request.client_file_id,
            &request.path,
            &parent_id,
            1,
            entries,
            response,
            next_id,
            total_bytes,
            terminator,
        )
    } else {
        Err("ENOSYS:Special files are not supported")
    }
}

#[allow(clippy::too_many_arguments)]
fn list_directory(
    session_id: &str,
    client_file_id: &str,
    directory: &Path,
    parent_id: &str,
    depth: usize,
    entries: &mut HashMap<String, ReceiveEntry>,
    response: &mut KittyFileTransferResponse,
    next_id: &mut u64,
    total_bytes: &mut u64,
    terminator: &str,
) -> Result<(), &'static str> {
    if depth > MAX_RECEIVE_DEPTH {
        return Err("EFBIG:Directory traversal is too deep");
    }
    let read_dir = fs::read_dir(directory).map_err(|_| "EIO:Could not list directory")?;
    for child in read_dir {
        let child = child.map_err(|_| "EIO:Could not list directory")?;
        let path = child.path();
        let metadata = match safe_symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err("ENOSYS:Links are not supported") => continue,
            Err(status) => return Err(status),
        };
        if metadata.is_file() {
            push_receive_entry(
                session_id,
                client_file_id,
                Some(parent_id),
                &path,
                FileKind::Regular,
                metadata.len(),
                entries,
                response,
                next_id,
                total_bytes,
                terminator,
            )?;
        } else if metadata.is_dir() {
            let entry_id = push_receive_entry(
                session_id,
                client_file_id,
                Some(parent_id),
                &path,
                FileKind::Directory,
                0,
                entries,
                response,
                next_id,
                total_bytes,
                terminator,
            )?;
            list_directory(
                session_id,
                client_file_id,
                &path,
                &entry_id,
                depth + 1,
                entries,
                response,
                next_id,
                total_bytes,
                terminator,
            )?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn push_receive_entry(
    session_id: &str,
    client_file_id: &str,
    parent_id: Option<&str>,
    path: &Path,
    kind: FileKind,
    size: u64,
    entries: &mut HashMap<String, ReceiveEntry>,
    response: &mut KittyFileTransferResponse,
    next_id: &mut u64,
    total_bytes: &mut u64,
    terminator: &str,
) -> Result<String, &'static str> {
    if entries.len() >= MAX_RECEIVE_FILES {
        return Err("EFBIG:Too many files in receive session");
    }
    if size > MAX_RECEIVE_FILE_BYTES {
        return Err("EFBIG:File is too large");
    }
    *total_bytes = total_bytes
        .checked_add(size)
        .ok_or("EFBIG:Receive session is too large")?;
    if *total_bytes > MAX_RECEIVE_SESSION_BYTES {
        return Err("EFBIG:Receive session is too large");
    }
    let key = path_key(path).ok_or("EINVAL:Invalid receive path")?;
    if let Some(existing) = entries.get(&key) {
        return Ok(existing.actual_file_id.clone());
    }
    let actual_file_id = format!("r{next_id}");
    *next_id += 1;
    let entry = ReceiveEntry {
        actual_file_id: actual_file_id.clone(),
        path: path.to_owned(),
        kind,
        size,
    };
    response.replies.push(file_metadata_reply(
        session_id,
        client_file_id,
        &entry,
        parent_id,
        terminator,
    ));
    entries.insert(key, entry);
    Ok(actual_file_id)
}

fn protocol_path(name: &str) -> Result<PathBuf, &'static str> {
    if name.is_empty() || name.len() > 4096 || name.as_bytes().contains(&0) {
        return Err("EINVAL:Invalid file transfer path");
    }

    let path = name
        .strip_prefix("~/")
        .unwrap_or(name)
        .trim_start_matches('/');
    let mut relative = PathBuf::new();
    for component in path.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            return Err("EINVAL:Invalid file transfer path");
        }
        if component.len() > 255
            || component.bytes().any(|byte| {
                byte.is_ascii_control()
                    || matches!(byte, b'\\' | b'*' | b'<' | b'>' | b'?' | b'|')
            })
        {
            return Err("EINVAL:Invalid file transfer path");
        }
        relative.push(component);
    }

    if relative.as_os_str().is_empty() {
        return Err("EINVAL:Invalid file transfer path");
    }
    Ok(relative)
}

fn destination_path(root: &Path, name: &str) -> Result<PathBuf, &'static str> {
    Ok(root.join(protocol_path(name)?))
}

fn receive_path(name: &str) -> Result<PathBuf, &'static str> {
    if name.is_empty() || name.len() > 4096 || name.as_bytes().contains(&0) {
        return Err("EINVAL:Invalid receive path");
    }
    let expanded = if name == "~" {
        std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map(PathBuf::from)
            .map_err(|_| "EINVAL:Cannot expand home directory")?
    } else if let Some(rest) = name.strip_prefix("~/") {
        std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map(|home| PathBuf::from(home).join(rest))
            .map_err(|_| "EINVAL:Cannot expand home directory")?
    } else {
        PathBuf::from(name)
    };
    if !expanded.is_absolute()
        || expanded
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err("EINVAL:Receive paths must be absolute");
    }
    Ok(expanded)
}

fn safe_symlink_metadata(path: &Path) -> Result<fs::Metadata, &'static str> {
    let mut current = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => current.push(prefix.as_os_str()),
            Component::RootDir => current.push(Path::new("/")),
            Component::CurDir => {}
            Component::ParentDir => return Err("EINVAL:Receive paths must be absolute"),
            Component::Normal(name) => {
                current.push(name);
                let metadata = fs::symlink_metadata(&current)
                    .map_err(|_| "ENOENT:Does not exist")?;
                if metadata.file_type().is_symlink() {
                    return Err("ENOSYS:Links are not supported");
                }
            }
        }
    }
    fs::symlink_metadata(path).map_err(|_| "ENOENT:Does not exist")
}

fn path_key(path: &Path) -> Option<String> {
    Some(path.to_str()?.to_owned())
}

fn cleanup_session(session: &KittyFileTransferSession) {
    if let KittyFileTransferSessionKind::Send(send) = &session.kind {
        let _ = fs::remove_dir_all(&send.staging_root);
    }
}

fn prepare_send_session(send: &mut SendSession) -> Result<(), &'static str> {
    if send.final_root.exists() || send.staging_root.exists() {
        return Err("EEXIST:Destination already exists");
    }
    fs::create_dir_all(&send.destination_root)
        .map_err(|_| "EIO:Could not create transfer destination")?;
    let staging_parent = send
        .staging_root
        .parent()
        .ok_or("EINVAL:Invalid staging directory")?;
    fs::create_dir_all(staging_parent)
        .map_err(|_| "EIO:Could not create transfer staging directory")?;
    fs::create_dir(&send.staging_root)
        .map_err(|_| "EIO:Could not create transfer staging directory")?;
    Ok(())
}
