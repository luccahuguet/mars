use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use base64::{engine::general_purpose, Engine as _};

use crate::performer::handler::{
    KittyFileTransfer, KittyFileTransferAction, KittyFileTransferCompression,
    KittyFileTransferFileType, KittyFileTransferTransmission,
};

const MAX_ACTIVE_SESSIONS: usize = 1;
const MAX_FILES: usize = 4096;
const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_SESSION_BYTES: u64 = 1024 * 1024 * 1024;
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
    pub destination_root: PathBuf,
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
    destination_root: PathBuf,
    final_root: PathBuf,
    staging_root: PathBuf,
    files: HashMap<String, KittyFileTransferFile>,
    total_written: u64,
    errored: bool,
}

#[derive(Debug)]
struct KittyFileTransferFile {
    kind: KittyFileTransferFileKind,
    expected_size: Option<u64>,
    written: u64,
    file: Option<File>,
    errored: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KittyFileTransferFileKind {
    Regular,
    Directory,
}

impl KittyFileTransferResponse {
    fn status(
        id: &str,
        file_id: Option<&str>,
        status: &str,
        size: Option<u64>,
        terminator: &str,
    ) -> Self {
        let mut response = Self::default();
        response.push_status(id, file_id, status, size, terminator);
        response
    }

    fn push_status(
        &mut self,
        id: &str,
        file_id: Option<&str>,
        status: &str,
        size: Option<u64>,
        terminator: &str,
    ) {
        self.replies
            .push(file_transfer_reply(id, file_id, status, size, terminator));
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
                &session.terminator,
            );
        }

        let result = self
            .sessions
            .get_mut(id)
            .filter(|session| session.approval == KittyFileTransferApproval::Pending)
            .ok_or("EPERM:No pending file transfer session")
            .and_then(prepare_session);

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
                    &session.terminator,
                )
            }
        }
    }

    pub(super) fn handle_transfer(
        &mut self,
        transfer: KittyFileTransfer,
        terminator: &str,
    ) -> KittyFileTransferResponse {
        match transfer.action {
            KittyFileTransferAction::Send => {
                self.start_send_session(transfer, terminator)
            }
            KittyFileTransferAction::Receive => KittyFileTransferResponse::status(
                &transfer.id,
                transfer.file_id.as_deref(),
                "EPERM:Yazelix-terminal receive/read transfers are not implemented",
                None,
                terminator,
            ),
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
                terminator,
            ),
            KittyFileTransferAction::Finish => self.finish(transfer, terminator),
        }
    }

    fn start_send_session(
        &mut self,
        transfer: KittyFileTransfer,
        terminator: &str,
    ) -> KittyFileTransferResponse {
        if transfer.transmission != KittyFileTransferTransmission::Simple
            || transfer.compression != KittyFileTransferCompression::None
        {
            return KittyFileTransferResponse::status(
                &transfer.id,
                transfer.file_id.as_deref(),
                "ENOSYS:Compressed and rsync file transfers are not implemented",
                None,
                terminator,
            );
        }
        if self.sessions.contains_key(&transfer.id) {
            return KittyFileTransferResponse::status(
                &transfer.id,
                None,
                "EEXIST:File transfer session id is already active",
                None,
                terminator,
            );
        }
        if self.sessions.len() >= MAX_ACTIVE_SESSIONS {
            return KittyFileTransferResponse::status(
                &transfer.id,
                None,
                "EBUSY:Another file transfer session is active",
                None,
                terminator,
            );
        }

        let destination_root = match default_destination_root() {
            Ok(root) => root,
            Err(status) => {
                return KittyFileTransferResponse::status(
                    &transfer.id,
                    None,
                    status,
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
                destination_root: destination_root.clone(),
                final_root,
                staging_root,
                files: HashMap::new(),
                total_written: 0,
                errored: false,
            },
        );

        KittyFileTransferResponse {
            replies: Vec::new(),
            approval_request: Some(KittyFileTransferApprovalRequest {
                id: transfer.id,
                destination_root,
            }),
        }
    }

    fn session_mut(
        &mut self,
        id: &str,
    ) -> Result<&mut KittyFileTransferSession, &'static str> {
        let session = self
            .sessions
            .get_mut(id)
            .ok_or("EPERM:No approved file transfer session")?;
        if session.approval != KittyFileTransferApproval::Approved {
            return Err("EPERM:File transfer session is waiting for approval");
        }
        Ok(session)
    }

    fn drop_pending_session(&mut self, id: &str) -> bool {
        if !matches!(
            self.sessions.get(id).map(|session| session.approval),
            Some(KittyFileTransferApproval::Pending)
        ) {
            return false;
        }
        if let Some(session) = self.sessions.remove(id) {
            cleanup_session(&session);
        }
        true
    }

    fn handle_file(
        &mut self,
        transfer: KittyFileTransfer,
        terminator: &str,
    ) -> KittyFileTransferResponse {
        let file_id = transfer.file_id.clone();
        let status = self.try_handle_file(&transfer);
        KittyFileTransferResponse::status(
            &transfer.id,
            file_id.as_deref(),
            status,
            None,
            terminator,
        )
    }

    fn try_handle_file(&mut self, transfer: &KittyFileTransfer) -> &'static str {
        if transfer.transmission != KittyFileTransferTransmission::Simple
            || transfer.compression != KittyFileTransferCompression::None
        {
            return "ENOSYS:Compressed and rsync file transfers are not implemented";
        }
        let Some(file_id) = &transfer.file_id else {
            return "EINVAL:Missing file id";
        };
        let Some(name) = &transfer.name else {
            return "EINVAL:Missing file name";
        };
        if self.drop_pending_session(&transfer.id) {
            return "EPERM:File transfer command arrived before approval";
        }

        let session = match self.session_mut(&transfer.id) {
            Ok(session) => session,
            Err(status) => return status,
        };
        if session.files.len() >= MAX_FILES {
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
        if transfer.size.unwrap_or(0) > MAX_FILE_BYTES {
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
                if fs::create_dir_all(parent).is_err()
                    || fs::create_dir_all(&path).is_err()
                {
                    session.errored = true;
                    return "EIO:Could not create directory";
                }
                session.files.insert(
                    file_id.clone(),
                    KittyFileTransferFile {
                        kind: KittyFileTransferFileKind::Directory,
                        expected_size: transfer.size,
                        written: 0,
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
                    KittyFileTransferFile {
                        kind: KittyFileTransferFileKind::Regular,
                        expected_size: transfer.size,
                        written: 0,
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

    fn handle_data(
        &mut self,
        transfer: KittyFileTransfer,
        terminator: &str,
        finish_file: bool,
    ) -> KittyFileTransferResponse {
        let file_id = transfer.file_id.clone();
        let (status, size) = self.try_handle_data(&transfer, finish_file);
        KittyFileTransferResponse::status(
            &transfer.id,
            file_id.as_deref(),
            status,
            size,
            terminator,
        )
    }

    fn try_handle_data(
        &mut self,
        transfer: &KittyFileTransfer,
        finish_file: bool,
    ) -> (&'static str, Option<u64>) {
        let Some(file_id) = &transfer.file_id else {
            return ("EINVAL:Missing file id", None);
        };
        if self.drop_pending_session(&transfer.id) {
            return ("EPERM:File transfer command arrived before approval", None);
        }
        let session = match self.session_mut(&transfer.id) {
            Ok(session) => session,
            Err(status) => return (status, None),
        };
        let Some(file) = session.files.get_mut(file_id) else {
            return ("EPERM:File was not started", None);
        };
        if file.errored {
            return ("EIO:File transfer already failed", Some(file.written));
        }
        if file.kind != KittyFileTransferFileKind::Regular {
            file.errored = true;
            session.errored = true;
            return (
                "EINVAL:Cannot write data to a directory",
                Some(file.written),
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
            if new_file_size > MAX_FILE_BYTES
                || file
                    .expected_size
                    .is_some_and(|expected| new_file_size > expected)
            {
                file.errored = true;
                session.errored = true;
                return ("EFBIG:File is too large", Some(file.written));
            }
            if new_session_size > MAX_SESSION_BYTES {
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
                terminator,
            );
        };

        let status = if session.approval != KittyFileTransferApproval::Approved {
            "EPERM:File transfer session is waiting for approval"
        } else if session.errored || session.files.values().any(|file| file.errored) {
            "EIO:Transfer session has errors"
        } else if session.files.values().any(|file| file.file.is_some()) {
            "EIO:Transfer contains unfinished files"
        } else if session.final_root.exists() {
            "EEXIST:Destination already exists"
        } else {
            for file in session.files.values_mut() {
                file.file = None;
            }
            match fs::rename(&session.staging_root, &session.final_root) {
                Ok(()) => "OK",
                Err(_) => "EIO:Could not commit transfer",
            }
        };

        if status != "OK" {
            cleanup_session(&session);
        }
        KittyFileTransferResponse::status(
            &session.id,
            None,
            status,
            Some(session.total_written),
            terminator,
        )
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
            terminator,
        )
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
        session.destination_root = destination_root.clone();
        session.final_root = destination_root.join(&segment);
        session.staging_root = destination_root.join(STAGING_DIR).join(segment);
    }
}

fn file_transfer_reply(
    id: &str,
    file_id: Option<&str>,
    status: &str,
    size: Option<u64>,
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
    reply.push_str(terminator);
    reply
}

fn default_destination_root() -> Result<PathBuf, &'static str> {
    if let Some(root) = std::env::var_os("XDG_DOWNLOAD_DIR") {
        return Ok(PathBuf::from(root).join("yazelix-terminal-transfers"));
    }
    if let Some(home) =
        std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))
    {
        return Ok(PathBuf::from(home)
            .join("Downloads")
            .join("yazelix-terminal-transfers"));
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

fn cleanup_session(session: &KittyFileTransferSession) {
    let _ = fs::remove_dir_all(&session.staging_root);
}

fn prepare_session(session: &mut KittyFileTransferSession) -> Result<(), &'static str> {
    if session.final_root.exists() || session.staging_root.exists() {
        return Err("EEXIST:Destination already exists");
    }
    fs::create_dir_all(&session.destination_root)
        .map_err(|_| "EIO:Could not create transfer destination")?;
    let staging_parent = session
        .staging_root
        .parent()
        .ok_or("EINVAL:Invalid staging directory")?;
    fs::create_dir_all(staging_parent)
        .map_err(|_| "EIO:Could not create transfer staging directory")?;
    fs::create_dir(&session.staging_root)
        .map_err(|_| "EIO:Could not create transfer staging directory")?;
    session.approval = KittyFileTransferApproval::Approved;
    Ok(())
}
