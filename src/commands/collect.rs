#![cfg(feature = "chrome")]

use anyhow::Result;
use clap::Args;
use headless_chrome::{
    protocol::cdp::Network::{
        events::ResponseReceivedEventParams, GetResponseBodyReturnObject, ResourceType,
    },
    Browser, LaunchOptionsBuilder,
};
use std::{fs::File, io::Write, path::PathBuf, sync::mpsc};
use kdam::term::Colorizer;

/// Collect playlists and subtitles from a website and save them locally.
#[derive(Debug, Clone, Args)]
#[clap(
    long_about = "Collect playlists and subtitles from a website and save them locally.\n\n\
Requires any one of these to be installed:\n\
1. chrome - https://www.google.com/chrome\n\
2. chromium - https://www.chromium.org/getting-involved/download-chromium\n\n\
Launch Google Chrome and collect .m3u8 (HLS), .mpd (Dash) and subtitles from a website and save them locally. \
This is done by reading the request response sent by chrome to server. \
This command might not work always as expected."
)]
pub struct Collect {
    /// http(s)://
    #[arg(required = true)]
    url: String,

    /// Change directory path for downloaded files.
    /// By default current working directory is used.
    #[arg(short, long)]
    pub directory: Option<PathBuf>,

    /// Launch browser without a window.
    #[arg(long)]
    headless: bool,
}

impl Collect {
    pub fn perform(&self) -> Result<()> {
        let (tx, rx) = mpsc::channel();
        ctrlc::set_handler(move || {
            tx.send(())
                .expect("could not send shutdown signal on channel.")
        })?;

        let browser = Browser::new(
            LaunchOptionsBuilder::default()
                .headless(self.headless)
                .build()?,
        )?;

        println!(
            "Launching browser {} a window.\n\
            Note that sometimes video starts playing but links are not captured.\n\
            If such condition occurs then try running the command again.",
            if self.headless { "without" } else { "with" },
        );

        let tab = browser.new_tab()?;
        let directory = self.directory.clone();

        if let Some(directory) = &directory {
            if !directory.exists() {
                std::fs::create_dir_all(directory)?;
            }
        }

        tab.register_response_handling(
            "vsd-collect",
            Box::new(move |params, get_response_body| {
                handler(params, get_response_body, &directory);
            }),
        )?;
        tab.navigate_to(&self.url)?;

        rx.recv()?;
        let _ = tab.deregister_response_handling("vsd-collect")?;

        if let Some(directory) = &self.directory {
            if std::fs::read_dir(directory)?.next().is_none() {
                println!(
                    "{} {}",
                    "Deleting".colorize("bold red"),
                    directory.to_string_lossy()
                );
                std::fs::remove_dir(directory)?;
            }
        }

        Ok(())
    }
}

fn handler(
    params: ResponseReceivedEventParams,
    get_response_body: &dyn Fn() -> Result<GetResponseBodyReturnObject>,
    directory: &Option<PathBuf>,
) {
    if params.Type == ResourceType::Xhr || params.Type == ResourceType::Fetch {
        let splitted_url = params.response.url.split('?').next().unwrap();

        if splitted_url.ends_with(".m3u")
            || splitted_url.ends_with(".m3u8")
            || splitted_url.ends_with(".mpd")
            || splitted_url.ends_with(".vtt")
            || splitted_url.ends_with(".srt")
        {
            let path = file_path(&params.response.url, directory);
            println!(
                "{} {} to {}",
                "Saving".colorize("bold green"),
                params.response.url,
                path.to_string_lossy().colorize("bold blue")
            );

            if let Ok(body) = get_response_body() {
                let mut file = File::create(path).unwrap();

                if body.base_64_encoded {
                    file.write_all(&openssl::base64::decode_block(&body.body).unwrap())
                        .unwrap();
                } else {
                    file.write_all(body.body.as_bytes()).unwrap();
                }
            } else {
                println!("Failed to save");
            }
        }
    }
}

fn file_path(url: &str, directory: &Option<PathBuf>) -> PathBuf {
    let mut filename = PathBuf::from(
        url.split('?')
            .next()
            .unwrap()
            .split('/')
            .last()
            .unwrap_or("undefined")
            .chars()
            .map(|x| match x {
                '<' | '>' | ':' | '\"' | '\\' | '|' | '?' => '_',
                _ => x,
            })
            .collect::<String>(),
    );

    let ext = filename
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("undefined")
        .to_owned();
    filename.set_extension("");
    let prefix = "vsd_collect";

    let mut path = PathBuf::from(format!("{}_{}.{}", prefix, filename.to_string_lossy(), ext));

    if let Some(directory) = directory {
        path = directory.join(path);
    }

    if path.exists() {
        for i in 1.. {
            path.set_file_name(format!(
                "{}_{}_({}).{}",
                prefix,
                filename.to_string_lossy(),
                i,
                ext
            ));

            if !path.exists() {
                return path;
            }
        }
    }

    path
}
