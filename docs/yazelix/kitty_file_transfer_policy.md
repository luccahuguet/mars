# Kitty File Transfer Policy

Status: implementation policy for OSC 5113 support.

Source spec: https://sw.kovidgoyal.net/kitty/file-transfer-protocol/

Kitty file transfer is useful because it moves files over the TTY when the TTY
is the only shared channel, such as nested SSH, serial links, or restricted
remote shells. It is also dangerous because terminal applications are not a
trusted authority for the user's local filesystem. Yazelix-terminal must treat
OSC 5113 as a user-authorized file exchange protocol, not as a parser feature.

## Threat Model

The terminal-side process may be connected to:

- a local shell running untrusted commands
- a remote shell over SSH
- nested terminal sessions or multiplexers
- malicious output produced by `cat`, logs, build tools, test fixtures, or
  copied terminal transcripts

Any of those can emit OSC 5113. Therefore, no OSC 5113 sequence may read from
or write to the user's filesystem unless the user has explicitly authorized the
session in the terminal UI or a deliberately configured trust mechanism accepts
that exact session.

## Default Policy

Default behavior before approval UI exists:

- parse support may exist
- every `send` or `receive` start request must be rejected with `EPERM`
- no file paths are opened
- no directories are created
- no symlinks or hardlinks are created
- no file data is emitted back to the PTY

This keeps application probing deterministic without granting filesystem
authority early.

Current remote-to-local behavior:

- `action=send` creates a pending session and asks the frontend for explicit
  approval before creating directories or files
- the approval prompt identifies that a terminal program wants to write files to
  this computer and shows the destination root
- the destination root is
  `$XDG_DOWNLOAD_DIR/yazelix-terminal-transfers` when set, otherwise
  `~/Downloads/yazelix-terminal-transfers`
- accepted sessions write only regular files and directories into a per-session
  staging directory and commit that staging directory only on `finish`
- rejected, canceled, errored, or uncommitted sessions do not expose partial
  files in the final destination root
- receive/read-local-files sessions collect the requested path list first, then
  ask for explicit approval before listing metadata or reading file contents
- the UI is denied by default when notification actions are unavailable

## Session Lifecycle

Yazelix-terminal should maintain explicit session state keyed by OSC 5113
`id`. A session starts only on `action=send` or `action=receive`.

Rules:

- reject duplicate active ids unless the previous session has reached a terminal
  state
- drop sessions that send file/data commands before terminal approval
- reject commands for unknown sessions
- reject malformed commands with a protocol error when responses are not quiet
- cancel must stop further reads/writes and remove temporary files
- finish commits only after all accepted files are complete

Quiet responses are honored only after authorization. `quiet=2` must not hide
security-relevant local UI, logs, or denial decisions.

## Writing Files To This Computer

This corresponds to the remote client starting `action=send`.

Current safe implementation:

- allow only regular files and directories
- accept RFC 1950 zlib-compressed regular file payloads and write only the
  decompressed bytes after enforcing the same file/session size limits
- use the user-approved default transfer root shown in the approval prompt
- create a per-session staging directory first
- write into staging, then atomically move into the destination root on
  `finish`
- never overwrite existing files by default
- report conflicts as errors

Path rules:

- accept only valid UTF-8 POSIX-style protocol paths
- treat absolute protocol paths as names under the approved transfer root, never
  as host-absolute paths
- reject `..` components after normalization
- reject empty components except for the root marker in protocol paths
- reject path components longer than 255 bytes
- reject total protocol paths longer than 4096 bytes
- reject paths that escape the selected destination root after canonicalization

Symlink and hardlink rules:

- initial support should reject symlinks and hardlinks with a clear unsupported
  status
- a later implementation may create symlinks only when the target is inside the
  same approved destination root
- hardlinks should remain unsupported until there is a clear product need

Metadata rules:

- preserve mtime when possible
- preserve ordinary user read/write/execute bits when safe
- ignore setuid, setgid, sticky bits, owner, group, ACLs, xattrs, and platform
  metadata until each has an explicit policy

## Reading Files From This Computer

This corresponds to the remote client starting `action=receive`.

Current safe implementation:

- require explicit user approval for every receive request
- show every requested path or a summarized tree preview before approval
- support zlib compression when the approved client requests compressed file
  data for an individual regular file
- never follow symlinks while traversing directories
- reject symlink/link and special-file requests
- reject device files, sockets, FIFOs, and special files
- send one file at a time as the spec requires
- stop cleanly on cancel
- enforce hard path-count, file-count, byte-count, file-size, and traversal-depth
  limits

The terminal must not let a remote process enumerate arbitrary local paths
silently. Directory recursion must have a bounded item count, byte count, and
depth limit before data is sent.

## Size And Resource Limits

Hard defaults should exist even if they become configurable later:

- maximum command payload size: 4096 bytes of decoded data per data packet
- maximum active sessions: 1 initially
- maximum files per session: bounded
- maximum total bytes per session: bounded
- maximum directory traversal depth: bounded
- maximum transfer duration without progress: bounded
- maximum in-memory buffered data: small; stream to staging files instead

If a request declares a size larger than the configured limit, reject it before
opening files.

Compression-specific limits:

- compressed incoming write chunks are buffered per regular file only until the
  `end_data` command, then decompressed with a hard output cap
- uncompressed output size is the authoritative storage/session size
- approved receive reads may read one already-size-checked regular file into
  memory before zlib compression, capped by the receive file size limit

## Bypass Policy

Kitty's spec supports bypassing interactive approval with a shared secret hash.
Yazelix-terminal should not enable generic SHA256 bypass by default because the
spec itself warns that hashing does not hide the password from brute force.

Policy:

- no bypass is accepted until there is an explicit config surface
- shared secrets must be opt-in and scoped to a trust context
- trust context should include at least session host/user identity where
  available
- bypass must never apply to receive/read-local-files requests by default
- bypass should be auditable and easy to revoke

If a future implementation adds Kitty-compatible public-key bypass, it should be
documented as a separate design decision.

## Logging And Audit

Yazelix-terminal should keep a small user-visible transfer history:

- session id
- direction
- accepted or denied
- destination root or requested root
- file count and byte count
- error/cancel reason
- timestamp

The log must not store file contents or secret bypass material.

## Implementation Sequence

1. Parser and session model
   - parse OSC 5113 key/value packets
   - emit deterministic `EPERM` for `send` and `receive`
   - add malformed packet tests and conformance fixtures

2. Approval UI and safe send-to-terminal writes
   - explicit default destination root shown in the approval prompt
   - staging directory
   - regular file and directory writes only
   - limits, cancel, finish, and cleanup

3. Safe receive-from-terminal reads
   - approval preview
   - bounded traversal
   - regular files and directories only
   - one-file-at-a-time streaming

4. Optional advanced features
   - zlib compression: implemented for regular-file send and receive data
   - symlinks constrained to the approved root: not implemented
   - rsync/delta transfer: not implemented
   - scoped bypass config: not implemented

## Non-Goals

- no automatic acceptance for remote shells
- no implicit write access to the current working directory
- no path adoption from terminal output without UI approval
- no support for arbitrary device/special files
- no hidden data exfiltration through quiet responses
