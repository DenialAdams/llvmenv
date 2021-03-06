//! Manage LLVM/Clang builds

use failure::err_msg;
use glob::glob;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

use config::*;
use error::*;

const LLVMENV_FN: &'static str = ".llvmenv";

#[derive(Debug)]
pub struct Build {
    name: String,             // name and id of build
    prefix: PathBuf,          // the path where the LLVM build realy exists
    llvmenv: Option<PathBuf>, // path of .llvmenv
}

impl Build {
    fn system() -> Self {
        Build {
            name: "system".into(),
            prefix: PathBuf::from("/usr"),
            llvmenv: None,
        }
    }

    pub fn from_path(path: &Path) -> Self {
        let name = path.file_name().unwrap().to_str().unwrap();
        Build {
            name: name.into(),
            prefix: path.to_owned(),
            llvmenv: None,
        }
    }

    pub fn from_name(name: &str) -> Self {
        if name == "system" {
            return Self::system();
        }
        Build {
            name: name.into(),
            prefix: data_dir().join(name),
            llvmenv: None,
        }
    }

    pub fn exists(&self) -> bool {
        self.prefix.is_dir()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn prefix(&self) -> &Path {
        &self.prefix
    }

    pub fn env_path(&self) -> Option<&Path> {
        match self.llvmenv {
            Some(ref path) => Some(path.as_path()),
            None => None,
        }
    }

    pub fn set_global(&self) -> Result<()> {
        self.set_local(&config_dir())
    }

    pub fn set_local(&self, path: &Path) -> Result<()> {
        let env = path.join(LLVMENV_FN);
        let mut f = fs::File::create(env)?;
        write!(f, "{}", self.name)?;
        info!("Write setting to {}", path.display());
        Ok(())
    }

    pub fn archive(&self, verbose: bool) -> Result<()> {
        let filename = format!("{}.tar.xz", self.name);
        Command::new("tar")
            .arg(if verbose { "cvf" } else { "cf" })
            .arg(&filename)
            .arg("--use-compress-prog=pixz")
            .arg(&self.name)
            .current_dir(data_dir())
            .check_run()?;
        println!("{}", data_dir().join(filename).display());
        Ok(())
    }

    pub fn version(&self) -> Result<(u32, u32, u32)> {
        let output = Command::new(self.prefix().join("bin").join("llvm-config"))
            .arg("--version")
            .output()?;
        let output = ::std::str::from_utf8(&output.stdout)?;
        let v = output
            .split(".")
            .map(|s| {
                s.trim()
                    .parse()
                    .map_err(|_| err_msg(format!("Cannot parse version: {}", s)))
            }).collect::<Result<Vec<_>>>()?;
        if v.len() != 3 {
            return Err(err_msg("Unexpected output from llvm-config"));
        }
        Ok((v[0], v[1], v[2]))
    }
}

fn local_builds() -> Result<Vec<Build>> {
    Ok(glob(&format!("{}/*/bin", data_dir().display()))?
        .filter_map(|path| {
            if let Ok(path) = path {
                path.parent().map(|path| Build::from_path(path))
            } else {
                None
            }
        }).collect())
}

pub fn builds() -> Result<Vec<Build>> {
    let mut bs = local_builds()?;
    bs.sort_by(|a, b| a.name.cmp(&b.name));
    bs.insert(0, Build::system());
    Ok(bs)
}

fn load_local_env(path: &Path) -> Result<Option<Build>> {
    let cand = path.join(LLVMENV_FN);
    if !cand.exists() {
        return Ok(None);
    }
    let mut f = fs::File::open(cand)?;
    let mut s = String::new();
    f.read_to_string(&mut s)?;
    let name = s.trim();
    let mut build = Build::from_name(name);
    if build.exists() {
        build.llvmenv = Some(path.into());
        Ok(Some(build))
    } else {
        Ok(None)
    }
}

fn load_global_env() -> Result<Option<Build>> {
    load_local_env(&config_dir())
}

pub fn seek_build() -> Result<Build> {
    // Seek .llvmenv from $PWD
    let mut path = env::current_dir()?;
    loop {
        if let Some(mut build) = load_local_env(&path)? {
            build.llvmenv = Some(path.join(LLVMENV_FN));
            return Ok(build);
        }
        path = match path.parent() {
            Some(path) => path.into(),
            None => break,
        };
    }
    // check global setting
    if let Some(mut build) = load_global_env()? {
        build.llvmenv = Some(config_dir().join(LLVMENV_FN));
        return Ok(build);
    }
    Ok(Build::system())
}

pub fn expand(archive: &Path, verbose: bool) -> Result<()> {
    if !archive.exists() {
        return Err(err_msg(format!(
            "Archive does not found: {}",
            archive.display()
        )));
    }
    Command::new("tar")
        .arg(if verbose { "xvf" } else { "xf" })
        .arg(archive)
        .current_dir(data_dir())
        .check_run()?;
    Ok(())
}
