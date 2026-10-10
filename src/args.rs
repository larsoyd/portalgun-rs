use std::path::PathBuf;

#[derive(Debug, PartialEq)]
pub struct Args {
    pub install: Option<String>,
    pub script: Option<PathBuf>,
    pub shots: Vec<f32>,
    pub shot_dir: PathBuf,
    pub quit_after: Option<f32>,
}

impl Default for Args {
    fn default() -> Self {
        Args {
            install: None,
            script: None,
            shots: Vec::new(),
            shot_dir: PathBuf::from("."),
            quit_after: None,
        }
    }
}

impl Args {
    pub fn parse(mut it: impl Iterator<Item = String>) -> Result<Args, String> {
        let mut a = Args::default();
        while let Some(flag) = it.next() {
            let mut value = || it.next().ok_or(format!("{flag} wants a value"));
            match flag.as_str() {
                "--install" => a.install = Some(value()?),
                "--script" => a.script = Some(value()?.into()),
                "--shot-dir" => a.shot_dir = value()?.into(),
                "--shot-at" => a.shots.push(seconds(&flag, &value()?)?),
                "--quit-after" => a.quit_after = Some(seconds(&flag, &value()?)?),
                _ => return Err(format!("unknown flag {flag}")),
            }
        }
        match (a.shots.iter().copied().reduce(f32::max), a.quit_after) {
            (Some(last), Some(quit)) if last >= quit => {
                Err(format!("--shot-at {last} comes after --quit-after {quit}"))
            }
            _ => Ok(a),
        }
    }
}

fn seconds(flag: &str, text: &str) -> Result<f32, String> {
    match text.parse::<f32>() {
        Ok(s) if s >= 0.0 && s.is_finite() => Ok(s),
        _ => Err(format!("{flag} wants seconds, got {text}")),
    }
}
