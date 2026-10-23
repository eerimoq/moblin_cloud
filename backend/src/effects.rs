pub struct Effect {
    pub id: &'static str,
    pub name: &'static str,
    pub kind: &'static str,
    pub content_type: &'static str,
    pub data: &'static [u8],
}

pub const EFFECTS: [Effect; 4] = [
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
];

pub fn effect(id: &str) -> Option<&'static Effect> {
    EFFECTS.iter().find(|effect| effect.id == id)
}
