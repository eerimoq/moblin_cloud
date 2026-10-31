use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::{Block, Padding, Paragraph, Wrap};

use crate::app::{Modal, Monitor};
use crate::layout::Leaf;
use crate::ui::{ACCENT, MUTED};

pub fn draw(frame: &mut Frame, area: Rect, monitor: &Monitor, leaf: &Leaf) {
    let twitch = monitor
        .page
        .as_ref()
        .and_then(|page| page.twitch.as_ref())
        .map(|twitch| twitch.login.clone());
    let lines = if let Some(Modal::Input(input)) = &monitor.modal
        && input.leaf == leaf.id
    {
        let mut lines = vec![
            Line::from("Web page address".fg(MUTED)),
            Line::from(vec![
                "> ".fg(ACCENT),
                input.text.clone().into(),
                "▏".fg(ACCENT),
            ]),
        ];
        if let Some(error) = &input.error {
            lines.push(Line::from(error.clone().red()));
        }
        lines
    } else if leaf.twitch_chat {
        match &twitch {
            Some(login) => vec![
                Line::from(format!("Twitch chat of {login}").bold()),
                Line::from(format!("twitch.tv/{login}/chat").underlined()),
                Line::default(),
                Line::from(vec![" o ".bold(), "opens it in your browser".fg(MUTED)]),
            ],
            None => vec![Line::from(
                "The streamer has no Twitch account linked.".fg(MUTED),
            )],
        }
    } else if let Some(url) = &leaf.url {
        vec![
            Line::from(url.clone().underlined()),
            Line::default(),
            Line::from(vec![" o ".bold(), "opens it in your browser".fg(MUTED)]),
        ]
    } else {
        let mut lines = vec![
            Line::from(
                "A terminal can't show web pages, but it keeps their address here.".fg(MUTED),
            ),
            Line::default(),
            Line::from(vec![" enter ".bold(), "sets an https:// address".fg(MUTED)]),
        ];
        if twitch.is_some() {
            lines.push(Line::from(vec![
                " t ".bold(),
                "shows the streamer's Twitch chat".fg(MUTED),
            ]));
        }
        lines
    };
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(Block::new().padding(Padding::horizontal(1))),
        area,
    );
}
