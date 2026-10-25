pub struct Icon {
    pub id: &'static str,
    pub moblin: &'static str,
    pub data: &'static [u8],
}

pub const ICONS: [Icon; 18] = [
    Icon {
        id: "plain",
        moblin: "AppIcon",
        data: include_bytes!("icons/plain.png"),
    },
    Icon {
        id: "san-diego",
        moblin: "AppIconSanDiego",
        data: include_bytes!("icons/san-diego.png"),
    },
    Icon {
        id: "king",
        moblin: "AppIconKing",
        data: include_bytes!("icons/king.png"),
    },
    Icon {
        id: "queen",
        moblin: "AppIconQueen",
        data: include_bytes!("icons/queen.png"),
    },
    Icon {
        id: "looking",
        moblin: "AppIconLooking",
        data: include_bytes!("icons/looking.png"),
    },
    Icon {
        id: "pixels",
        moblin: "AppIconPixels",
        data: include_bytes!("icons/pixels.png"),
    },
    Icon {
        id: "heart",
        moblin: "AppIconHeart",
        data: include_bytes!("icons/heart.png"),
    },
    Icon {
        id: "pink",
        moblin: "AppIconPink",
        data: include_bytes!("icons/pink.png"),
    },
    Icon {
        id: "happy",
        moblin: "AppIconHappy",
        data: include_bytes!("icons/happy.png"),
    },
    Icon {
        id: "millionaire",
        moblin: "AppIconMillionaire",
        data: include_bytes!("icons/millionaire.png"),
    },
    Icon {
        id: "billionaire",
        moblin: "AppIconBillionaire",
        data: include_bytes!("icons/billionaire.png"),
    },
    Icon {
        id: "trillionaire",
        moblin: "AppIconTrillionaire",
        data: include_bytes!("icons/trillionaire.png"),
    },
    Icon {
        id: "tetris",
        moblin: "AppIconTetris",
        data: include_bytes!("icons/tetris.png"),
    },
    Icon {
        id: "tub",
        moblin: "AppIconTub",
        data: include_bytes!("icons/tub.png"),
    },
    Icon {
        id: "goblin",
        moblin: "AppIconGoblin",
        data: include_bytes!("icons/goblin.png"),
    },
    Icon {
        id: "goblina",
        moblin: "AppIconGoblina",
        data: include_bytes!("icons/goblina.png"),
    },
    Icon {
        id: "ireland",
        moblin: "AppIconIreland",
        data: include_bytes!("icons/ireland.png"),
    },
    Icon {
        id: "peru",
        moblin: "AppIconPeru",
        data: include_bytes!("icons/peru.png"),
    },
];

pub fn icon(id: &str) -> Option<&'static Icon> {
    ICONS.iter().find(|icon| icon.id == id)
}

pub fn of_moblin(moblin: &str) -> Option<&'static Icon> {
    ICONS.iter().find(|icon| icon.moblin == moblin)
}
