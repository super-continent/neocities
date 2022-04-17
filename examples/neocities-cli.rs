use std::{fs::File, io::Read, path::PathBuf};

use clap::{Parser, Subcommand};
use neocities::{ListEntry, Neocities};
use walkdir::WalkDir;

#[tokio::main]
async fn main() {
    match run().await {
        Err(e) => {
            println!("error: {}", e);
            std::process::exit(1);
        }
        _ => {}
    }
}

#[derive(Debug, Parser)]
struct Cli {
    /// The API key for your Neocities site.
    /// If a key is not passed through this flag,
    /// the API key will be taken from an environment variable.
    #[clap(short, long, env = "NEOCITIES_KEY")]
    key: Option<String>,
    /// Your Neocities site name
    #[clap(short, long)]
    username: Option<String>,
    /// Your Neocities account password
    #[clap(short, long)]
    password: Option<String>,
    #[clap(subcommand)]
    subcommand: ApiCmd,
}

#[derive(Debug, Subcommand)]
enum ApiCmd {
    /// Get info about a Neocities site
    Info { site_name: String },
    /// List the files in
    List { directory: Option<String> },
    /// Gets the API key for your site or generates it if one does not exist
    Key,
    /// Delete a list of files or directories from your site
    Delete { paths: Vec<String> },
    /// Delete ALL FILES recursively from your Neocities site.
    /// NOTE: This will not delete index.html, as that file is required
    DeleteAll,
    /// Upload files to the authenticated Neocities site
    Upload {
        /// The path to the file you want to upload
        file_path: PathBuf,
        /// Specify the path and name of the file being uploaded.
        /// This will create any specified folders that dont exist in the sites filesystem.
        /// If not specified the file will be uploaded to the root of the site
        #[clap(short, long)]
        custom_path: Option<String>,
    },
    /// This command uploads all files recursively within a specified directory
    /// The specified directory will be treated as the root
    UploadAll { root: PathBuf },
}

async fn run() -> Result<(), String> {
    let cli = Cli::parse();

    let api = if let (Some(username), Some(password)) = (cli.username, cli.password) {
        Neocities::login(username, password)
    } else if let Some(key) = cli.key {
        Neocities::new(key)
    } else {
        return Err("No login specified!".into());
    };

    match cli.subcommand {
        ApiCmd::Info { site_name } => {
            let info = api.info(&site_name).await.map_err(|e| e.to_string())?;
            println!("Site info for {}:", info.site_name);
            println!(
                "Custom Domain: {}",
                info.domain.map_or("None".to_string(), |d| d)
            );
            println!("Created at: {}", info.created_at);
            println!("Last updated: {}", info.last_updated);
            println!("Views: {}", info.hits);

            let tags = info
                .tags
                .iter()
                .fold("".to_string(), |out, name| out + name + ", ");
            println!("Tags: {}", tags);
        }
        ApiCmd::List { directory } => {
            let files = api
                .list(directory.unwrap_or("".to_string()))
                .await
                .map_err(|e| e.to_string())?;

            for entry in files {
                match entry {
                    ListEntry::File {
                        path,
                        size,
                        updated_at,
                        sha1_hash,
                    } => {
                        println!("File: {}", path);
                        println!("Size: {}", size);
                        println!("Updated at: {}", updated_at);
                        println!("SHA-1: {}", sha1_hash);
                    }
                    _ => {}
                }
            }
        }
        ApiCmd::Key => {
            let key = api.key().await.map_err(|e| e.to_string())?;
            println!("Neocities Key: {}", key);
        }
        ApiCmd::Delete { paths } => {
            let res = api.delete(paths).await.map_err(|e| e.to_string())?;
            println!("{}", res);
        }
        ApiCmd::DeleteAll => {
            let list = api.list("").await.map_err(|e| e.to_string())?;

            for entry in &list {
                match entry {
                    ListEntry::File {
                        path,
                        size: _,
                        updated_at: _,
                        sha1_hash: _,
                    } => {
                        if path == "index.html" {
                            continue;
                        }

                        let res = api.delete([path.clone()]).await;

                        if res.is_err() {
                            println!("Failed to delete `{}`", path);
                        }
                    }
                    _ => {}
                }
            }

            for entry in &list {
                match entry {
                    ListEntry::Directory {
                        path,
                        updated_at: _,
                    } => {
                        let res = api.delete([path.clone()]).await;

                        if res.is_err() {
                            //println!("Failed to delete `{}`", path);
                        }
                    }
                    _ => {}
                }
            }
        }
        ApiCmd::Upload {
            file_path,
            custom_path,
        } => {
            if !file_path.is_file() {
                return Err("File either does not exist or is a directory".to_string());
            }

            let mut file_vec = Vec::new();
            File::open(&file_path)
                .map_err(|e| e.to_string())?
                .read_to_end(&mut file_vec)
                .map_err(|e| e.to_string())?;

            let file_name = if let Some(name) = file_path.file_name() {
                name.to_string_lossy().to_string()
            } else {
                "..".into()
            };

            api.upload(custom_path.unwrap_or(file_name), file_vec)
                .await
                .map_err(|e| e.to_string())?;
        }
        ApiCmd::UploadAll { root } => {
            for entry in WalkDir::new(&root) {
                let entry = entry.map_err(|e| e.to_string())?;
                let path = entry.path();
                let neocities_path = path
                    .strip_prefix(&root)
                    .map_err(|e| e.to_string())?
                    .to_string_lossy()
                    .to_string();
                let neocities_path = neocities_path.replace("\\", "/");

                if path.is_dir() {
                    continue;
                }

                let mut file_vec = Vec::new();
                File::open(&path)
                    .map_err(|e| e.to_string())?
                    .read_to_end(&mut file_vec)
                    .map_err(|e| e.to_string())?;

                api.upload(neocities_path, file_vec)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }
    }

    Ok(())
}
