use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;
use tokio::time::timeout;

use crate::MediaError;

const HARD_MAX_OUTPUT_BYTES: u64 = 256 * 1024 * 1024;

/// Closed audio output formats required by `OneBot` 11.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AudioFormat {
    /// MPEG audio layer III.
    Mp3,
    /// Adaptive multi-rate audio.
    Amr,
    /// Windows Media Audio.
    Wma,
    /// MPEG-4 audio container.
    M4a,
    /// Speex audio.
    Spx,
    /// Ogg container.
    Ogg,
    /// Waveform audio.
    Wav,
    /// Free Lossless Audio Codec.
    Flac,
}

impl AudioFormat {
    const fn extension(self) -> &'static str {
        match self {
            Self::Mp3 => "mp3",
            Self::Amr => "amr",
            Self::Wma => "wma",
            Self::M4a => "m4a",
            Self::Spx => "spx",
            Self::Ogg => "ogg",
            Self::Wav => "wav",
            Self::Flac => "flac",
        }
    }
}

/// Explicit process and output bounds for one conversion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TranscodePolicy {
    timeout: Duration,
    maximum_output_bytes: u64,
}

impl TranscodePolicy {
    /// Creates a bounded policy.
    ///
    /// # Errors
    ///
    /// Returns an error unless timeout is one second through ten minutes and output is nonzero
    /// within the compiled hard maximum.
    pub fn new(timeout: Duration, maximum_output_bytes: u64) -> Result<Self, MediaError> {
        if !(Duration::from_secs(1)..=Duration::from_mins(10)).contains(&timeout)
            || maximum_output_bytes == 0
            || maximum_output_bytes > HARD_MAX_OUTPUT_BYTES
        {
            return Err(MediaError::Configuration);
        }
        Ok(Self {
            timeout,
            maximum_output_bytes,
        })
    }
}

/// No-shell `FFmpeg` adapter with a fixed argument shape.
#[derive(Clone, Debug)]
pub struct FfmpegTranscoder {
    executable: PathBuf,
    policy: TranscodePolicy,
}

impl FfmpegTranscoder {
    /// Creates a converter using an explicitly configured executable path.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty path.
    pub fn new(executable: PathBuf, policy: TranscodePolicy) -> Result<Self, MediaError> {
        if executable.as_os_str().is_empty() {
            return Err(MediaError::Configuration);
        }
        Ok(Self { executable, policy })
    }

    /// Converts one local file to a caller-selected output path and closed format.
    ///
    /// The process has no shell, inherited input, or inherited output. The caller is responsible
    /// for constraining concurrent conversions at its account service boundary.
    ///
    /// # Errors
    ///
    /// Returns an error for non-regular inputs, existing/invalid outputs, timeout, nonzero exit,
    /// or an output beyond the configured byte bound.
    pub async fn transcode(
        &self,
        input: &Path,
        output: &Path,
        format: AudioFormat,
    ) -> Result<(), MediaError> {
        if !tokio::fs::metadata(input)
            .await
            .map_err(|_error| MediaError::Conversion)?
            .is_file()
            || output.as_os_str().is_empty()
            || tokio::fs::try_exists(output)
                .await
                .map_err(|_error| MediaError::Conversion)?
        {
            return Err(MediaError::Conversion);
        }
        let output_directory = output.parent().ok_or(MediaError::Conversion)?;
        let staged = tempfile::Builder::new()
            .prefix(".lirvena-media-")
            .tempfile_in(output_directory)
            .map_err(|_error| MediaError::Conversion)?;
        let mut child = Command::new(&self.executable)
            .arg("-nostdin")
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-i")
            .arg(input)
            .arg("-map_metadata")
            .arg("-1")
            .arg("-vn")
            .arg("-f")
            .arg(format.extension())
            .arg("-y")
            .arg(staged.path())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_error| MediaError::Conversion)?;
        let status = if let Ok(result) = timeout(self.policy.timeout, child.wait()).await {
            result.map_err(|_error| MediaError::Conversion)?
        } else {
            child
                .kill()
                .await
                .map_err(|_error| MediaError::Conversion)?;
            return Err(MediaError::Conversion);
        };
        if !status.success() {
            return Err(MediaError::Conversion);
        }
        let metadata = tokio::fs::metadata(staged.path())
            .await
            .map_err(|_error| MediaError::Conversion)?;
        if !metadata.is_file() || metadata.len() > self.policy.maximum_output_bytes {
            return Err(MediaError::Conversion);
        }
        staged
            .persist_noclobber(output)
            .map_err(|_error| MediaError::Conversion)?;
        Ok(())
    }
}
