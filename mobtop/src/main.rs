mod account;
mod app;
mod backend;
mod controls;
mod feed;
mod layout;
mod menu;
mod panes;
mod preview;
mod session;
mod sign_in;
mod sse;
mod ui;

use anyhow::Result;
use clap::Parser;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyEventKind};
use crossterm::execute;
use futures_util::StreamExt;
use ratatui_image::picker::{Picker, ProtocolType};
use tokio::sync::mpsc;

use crate::app::{App, Message};

#[derive(Parser)]
#[command(version, about)]
struct Args {
    #[arg(
        long,
        env = "MOBTOP_BACKEND_URL",
        default_value = "https://cloud-api.moblin.app"
    )]
    backend_url: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let mut terminal = ratatui::init();
    let picker = Picker::from_query_stdio()
        .ok()
        .filter(|picker| picker.protocol_type() != ProtocolType::Halfblocks);
    let (sender, mut receiver) = mpsc::unbounded_channel();
    tokio::spawn({
        let sender = sender.clone();
        async move {
            let mut events = EventStream::new();
            while let Some(Ok(event)) = events.next().await {
                let message = match event {
                    Event::Key(key) if key.kind == KeyEventKind::Press => Message::Key(key),
                    Event::Mouse(mouse) => Message::Mouse(mouse),
                    Event::Resize(..) => Message::Redraw,
                    _ => continue,
                };
                if sender.send(message).is_err() {
                    return;
                }
            }
        }
    });
    tokio::spawn({
        let sender = sender.clone();
        async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                if sender.send(Message::Tick).is_err() {
                    return;
                }
            }
        }
    });
    let mut app = App::new(&args.backend_url, sender, picker);
    execute!(std::io::stdout(), EnableMouseCapture)?;
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = execute!(std::io::stdout(), DisableMouseCapture);
        hook(info);
    }));
    let result = async {
        terminal.draw(|frame| ui::draw(frame, &app))?;
        while let Some(message) = receiver.recv().await {
            if app.update(message) {
                break;
            }
            terminal.draw(|frame| ui::draw(frame, &app))?;
            app.encode_previews();
        }
        Ok(())
    }
    .await;
    if let Some(saving) = app.save() {
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), saving).await;
    }
    let _ = execute!(std::io::stdout(), DisableMouseCapture);
    ratatui::restore();
    result
}
