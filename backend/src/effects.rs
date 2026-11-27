pub struct Effect {
    pub id: &'static str,
    pub name: &'static str,
    pub kind: &'static str,
    pub content_type: &'static str,
    pub data: &'static [u8],
}

pub const EFFECTS: [Effect; 13] = [
    Effect {
        id: "fart",
        name: "Fart",
        kind: "sound",
        content_type: "audio/mpeg",
        data: include_bytes!("effects/fart.mp3"),
    },
    Effect {
        id: "fart-2",
        name: "Fart 2",
        kind: "sound",
        content_type: "audio/mpeg",
        data: include_bytes!("effects/fart-2.mp3"),
    },
    Effect {
        id: "bad-chili-fart",
        name: "Bad chili fart",
        kind: "sound",
        content_type: "audio/mpeg",
        data: include_bytes!("effects/bad-chili-fart.mp3"),
    },
    Effect {
        id: "perfect-fart",
        name: "Perfect fart",
        kind: "sound",
        content_type: "audio/mpeg",
        data: include_bytes!("effects/perfect-fart.mp3"),
    },
    Effect {
        id: "notification-2",
        name: "Notification 2",
        kind: "sound",
        content_type: "audio/mpeg",
        data: include_bytes!("effects/notification-2.mp3"),
    },
    Effect {
        id: "boing",
        name: "Boing",
        kind: "sound",
        content_type: "audio/mpeg",
        data: include_bytes!("effects/boing.mp3"),
    },
    Effect {
        id: "cash-register",
        name: "Cash register",
        kind: "sound",
        content_type: "audio/mpeg",
        data: include_bytes!("effects/cash-register.mp3"),
    },
    Effect {
        id: "dingaling",
        name: "Dingaling",
        kind: "sound",
        content_type: "audio/mpeg",
        data: include_bytes!("effects/dingaling.mp3"),
    },
    Effect {
        id: "level-up",
        name: "Level up",
        kind: "sound",
        content_type: "audio/mpeg",
        data: include_bytes!("effects/level-up.mp3"),
    },
    Effect {
        id: "notification",
        name: "Notification",
        kind: "sound",
        content_type: "audio/mpeg",
        data: include_bytes!("effects/notification.mp3"),
    },
    Effect {
        id: "sfx-magic",
        name: "SFX magic",
        kind: "sound",
        content_type: "audio/mpeg",
        data: include_bytes!("effects/sfx-magic.mp3"),
    },
    Effect {
        id: "whoosh",
        name: "Whoosh",
        kind: "sound",
        content_type: "audio/mpeg",
        data: include_bytes!("effects/whoosh.mp3"),
    },
    Effect {
        id: "coin-dropping",
        name: "Coin dropping",
        kind: "sound",
        content_type: "audio/mpeg",
        data: include_bytes!("effects/coin-dropping.mp3"),
    },
];

pub fn effect(id: &str) -> Option<&'static Effect> {
    EFFECTS.iter().find(|effect| effect.id == id)
}
