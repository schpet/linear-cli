use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

pub const MAX_CONFIG_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OsFamily {
    Unix,
    Windows,
}

#[derive(Clone, Eq, PartialEq)]
pub struct ConfigInputs {
    pub cwd: PathBuf,
    pub os: OsFamily,
    pub process_env: BTreeMap<String, String>,
}

impl ConfigInputs {
    pub fn env(&self, name: &str) -> Option<&str> {
        self.process_env.get(name).map(String::as_str)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GitProbeResult {
    SpawnFailure,
    Completed { success: bool, stdout: String },
    Failed(GitProbeError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GitIoStage {
    Poll,
    ReadStdout,
    Reap,
}

impl fmt::Display for GitIoStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Poll => "status check",
            Self::ReadStdout => "stdout read",
            Self::Reap => "child cleanup",
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GitProbeError {
    Timeout,
    Oversize,
    InvalidUtf8,
    MalformedStdout,
    Io {
        stage: GitIoStage,
        kind: io::ErrorKind,
    },
}

impl fmt::Display for GitProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timeout => f.write_str("Git root lookup timed out"),
            Self::Oversize => f.write_str("Git root lookup output exceeded 65536 bytes"),
            Self::InvalidUtf8 => f.write_str("Git root lookup output is not UTF-8"),
            Self::MalformedStdout => f.write_str("Git root lookup returned an invalid path"),
            Self::Io { stage, kind } => write!(f, "Git root lookup {stage} failed: {kind}"),
        }
    }
}

impl std::error::Error for GitProbeError {}

pub trait GitRootProbe {
    fn probe(&self) -> GitProbeResult;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileKind {
    Regular,
    Directory,
    Other,
}

pub trait FileSource {
    fn kind(&self, path: &Path) -> io::Result<Option<FileKind>>;
    fn read_bounded(&self, path: &Path, max_bytes: u64) -> io::Result<Vec<u8>>;
}

pub struct RealFileSource;

impl FileSource for RealFileSource {
    fn kind(&self, path: &Path) -> io::Result<Option<FileKind>> {
        match fs::metadata(path) {
            Ok(meta) if meta.is_file() => Ok(Some(FileKind::Regular)),
            Ok(meta) if meta.is_dir() => Ok(Some(FileKind::Directory)),
            Ok(_) => Ok(Some(FileKind::Other)),
            Err(error) if absent(&error) => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn read_bounded(&self, path: &Path, max_bytes: u64) -> io::Result<Vec<u8>> {
        let file = File::open(path)?;
        let mut bytes = Vec::new();
        file.take(max_bytes.saturating_add(1))
            .read_to_end(&mut bytes)?;
        Ok(bytes)
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct RawConfigFile {
    pub path: PathBuf,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Eq, PartialEq)]
pub enum ReadCandidate {
    Absent,
    Contents(RawConfigFile),
    TooLarge { path: PathBuf },
    Poisoned { path: PathBuf, reason: String },
}

pub fn read_config_candidate(files: &impl FileSource, path: &Path) -> ReadCandidate {
    match files.kind(path) {
        Ok(None) => ReadCandidate::Absent,
        Ok(Some(FileKind::Regular)) => match files.read_bounded(path, MAX_CONFIG_BYTES) {
            Ok(bytes) if u64::try_from(bytes.len()).is_ok_and(|len| len <= MAX_CONFIG_BYTES) => {
                ReadCandidate::Contents(RawConfigFile {
                    path: path.to_owned(),
                    bytes,
                })
            }
            Ok(_) => ReadCandidate::TooLarge {
                path: path.to_owned(),
            },
            Err(error) if absent(&error) => ReadCandidate::Absent,
            Err(error) => ReadCandidate::Poisoned {
                path: path.to_owned(),
                reason: error.kind().to_string(),
            },
        },
        Ok(Some(FileKind::Directory | FileKind::Other)) => ReadCandidate::Poisoned {
            path: path.to_owned(),
            reason: "not a regular file".to_owned(),
        },
        Err(error) => ReadCandidate::Poisoned {
            path: path.to_owned(),
            reason: error.kind().to_string(),
        },
    }
}

pub(crate) fn absent(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
    )
}

/// Normalize lexical . and .. without following symlinks or touching the filesystem.
pub(crate) fn lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match out.components().next_back() {
                Some(Component::Normal(_)) => {
                    out.pop();
                }
                Some(Component::RootDir) => {}
                Some(Component::Prefix(_))
                | Some(Component::ParentDir | Component::CurDir)
                | None => out.push(component.as_os_str()),
            },
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                out.push(component.as_os_str());
            }
        }
    }
    out
}
