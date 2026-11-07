//! The lobby and the match as panels: the Seats panel docks left, the Match
//! panel right, and the lineup turns in the open middle (`crate::dock`).
//! Every rule here is a question put to the room; this file only words the
//! answer.

use super::App;
use crate::game::Match;
use crate::hud;
use crate::lobby::{Role, Who, EATS, MAX_SEATS, MINUTES, SHEEP};
use crate::map::MapSize;
use crate::online::room::{Kind, Room};
use crate::view::{Choice, Item, Panel};

/// The three roles in the order the select shows them.
const ROLES: [Role; 3] = [Role::Sheepdog, Role::Wolf, Role::Random];

impl App {
    fn role_word(&self, role: Role) -> String {
        self.strings.get(&format!("role.{}", role.key()))
    }

    /// A seat's name as it reads: the bot's word, or the person's name, or
    /// "Player 3" for a person who has given none.
    pub(super) fn seat_name(&self, room: &Room, n: usize) -> String {
        match &room.lobby.seats[n].who {
            Who::Empty => self.strings.get("seat.empty"),
            Who::Bot => self.strings.get("seat.bot"),
            Who::Person { name, .. } if name.is_empty() => self
                .strings
                .fill("seat.default_name", &[("n", (n + 1).to_string())]),
            Who::Person { name, .. } => name.clone(),
        }
    }

    fn role_select(&self, n: usize, role: Role) -> Item {
        Item::Select {
            id: format!("seat.{n}.role"),
            label: String::new(),
            value: role.key().to_string(),
            options: ROLES
                .iter()
                .map(|r| Choice {
                    value: r.key().to_string(),
                    text: self.role_word(*r),
                })
                .collect(),
            disabled: false,
        }
    }

    fn seat_item(&self, room: &Room, n: usize) -> Item {
        let seat = &room.lobby.seats[n];
        let mine = room.lobby.seat_of(&room.client) == Some(n);
        let host = room.is_host();
        let (is_bot, is_empty) = (seat.who == Who::Bot, seat.who == Who::Empty);
        let chooses = mine || (host && is_bot);
        let mut controls = Vec::new();
        if chooses {
            controls.push(self.role_select(n, seat.role));
        }
        if host && is_bot {
            controls.push(self.quiet(&format!("seat.{n}.no_bot"), "seat.remove_bot"));
        }
        if host && is_empty {
            controls.push(self.quiet(&format!("seat.{n}.bot"), "seat.add_bot"));
        }
        let is_host_seat = matches!(&seat.who, Who::Person { id, .. } if *id == room.lobby.host);
        let ready = matches!(&seat.who, Who::Person { ready: true, .. });
        let mut notes = Vec::new();
        if mine {
            notes.push(self.strings.get("seat.you"));
        } else if is_host_seat {
            notes.push(self.strings.get("seat.host"));
        }
        if ready {
            notes.push(self.strings.get("seat.is_ready"));
        }
        Item::Seat {
            id: format!("seat.{n}"),
            look: if is_empty {
                "empty".to_string()
            } else {
                seat.role.key().to_string()
            },
            name: self.seat_name(room, n),
            note: notes.join(" \u{b7} "),
            me: mine,
            ready,
            role: if chooses || is_empty {
                String::new()
            } else {
                self.role_word(seat.role)
            },
            controls,
        }
    }

    /// A pasted code to send, and its Copy button; nothing until it is made.
    fn code_out(&self, id: &str, label: &str, code: &str) -> Vec<Item> {
        if code.is_empty() {
            return Vec::new();
        }
        vec![
            Item::CodeBox {
                id: id.into(),
                label: self.strings.get(label),
                value: code.to_string(),
                read_only: true,
            },
            Item::Copy {
                id: format!("{id}.copy"),
                label: String::new(),
                value: String::new(),
                copy: code.to_string(),
                text: self.strings.get("lobby.copy_code"),
            },
        ]
    }

    /// The host's side of meeting by pasted codes.
    fn invite_items(&self, room: &Room) -> Vec<Item> {
        let invite = room.pasted_code().unwrap_or_default();
        let mut items = self.code_out("online.offer_out", "online.invite_code", invite);
        items.push(Item::CodeBox {
            id: "online.answer_in".into(),
            label: self.strings.get("online.reply_code"),
            value: self.typed_reply.clone(),
            read_only: false,
        });
        items.push(Item::Button {
            id: "online.accept_answer".into(),
            text: self.strings.get("online.connect"),
            quiet: false,
            disabled: self.typed_reply.trim().is_empty(),
        });
        items
    }

    /// The joiner's side: stays up until the host's lobby is heard.
    pub(super) fn paste_panels(&self, room: Option<&Room>) -> Vec<Panel> {
        let reply = room.and_then(Room::pasted_code).unwrap_or_default();
        let busy = room.is_some_and(Room::making_code);
        let mut items = vec![
            Item::CodeBox {
                id: "online.offer_in".into(),
                label: self.strings.get("online.invite_code"),
                value: self.typed_invite.clone(),
                read_only: false,
            },
            Item::Button {
                id: "online.make_answer".into(),
                text: self.strings.get("online.make_reply"),
                quiet: false,
                disabled: busy || self.typed_invite.trim().is_empty(),
            },
        ];
        items.extend(self.code_out("online.answer_out", "online.reply_code", reply));
        items.push(self.quiet("online.paste_back", "menu.back"));
        vec![Panel::new("online", self.strings.get("online.paste_join"), items)]
    }

    fn seats_panel(&self, room: &Room) -> Panel {
        let mut items = Vec::new();
        if room.pasted_code().is_some() {
            if room.is_host() {
                items.extend(self.invite_items(room));
            }
        } else if room.kind != Kind::Solo {
            items.push(Item::Copy {
                id: "lobby.copy_code".into(),
                label: self.strings.get("lobby.code"),
                value: room.code.clone(),
                copy: room.code.clone(),
                text: self.strings.get("lobby.copy_code"),
            });
            items.push(Item::Copy {
                id: "lobby.copy_link".into(),
                label: String::new(),
                value: String::new(),
                copy: format!("{}#room={}", self.page_url, room.code),
                text: self.strings.get("lobby.copy_link"),
            });
        }
        items.extend((0..MAX_SEATS).map(|n| self.seat_item(room, n)));
        let heading = if room.kind == Kind::Solo {
            "lobby.solo_heading"
        } else {
            "lobby.seats"
        };
        Panel::new("seats", self.strings.get(heading), items)
    }

    fn number(&self, id: &str, value: i32, range: (i32, i32), disabled: bool) -> Item {
        Item::Number {
            id: id.into(),
            label: self.strings.get(id),
            value: value as u32,
            min: range.0 as u32,
            max: range.1 as u32,
            disabled,
        }
    }

    fn switch(&self, id: &str, value: bool, disabled: bool) -> Item {
        Item::Toggle {
            id: id.into(),
            label: self.strings.get(id),
            value,
            disabled,
        }
    }

    fn match_panel(&self, room: &Room) -> Panel {
        let host = room.is_host();
        let s = &room.lobby.settings;
        let mut items = vec![
            self.number("lobby.minutes", s.match_minutes, MINUTES, !host),
            Item::Select {
                id: "lobby.map_size".into(),
                label: self.strings.get("lobby.map_size"),
                value: s.map_size.key().to_string(),
                options: MapSize::ALL
                    .iter()
                    .map(|size| Choice {
                        value: size.key().to_string(),
                        text: self.strings.get(&format!("map_size.{}", size.key())),
                    })
                    .collect(),
                disabled: !host,
            },
            self.number("lobby.sheep", s.sheep, SHEEP, !host),
            self.number(
                "lobby.eats",
                s.eats_required,
                (EATS.0, s.sheep.min(EATS.1)),
                !host,
            ),
            self.switch("lobby.powerups", s.powerups, !host),
            self.switch("lobby.sudden", s.sudden_death, !host),
        ];
        if host {
            items.push(self.quiet("lobby.defaults", "lobby.defaults"));
        }
        if room.kind != Kind::Solo {
            items.push(Item::Text {
                id: "lobby.name".into(),
                label: self.strings.get("lobby.name"),
                value: self.save.name.clone(),
                max: crate::lobby::MAX_NAME_CHARS as u32,
                placeholder: self.strings.get("lobby.name_hint"),
            });
        }
        let problem = room.lobby.start_problem();
        if host {
            items.push(Item::Button {
                id: "lobby.start".into(),
                text: self.strings.get("lobby.start"),
                quiet: false,
                disabled: problem.is_some(),
            });
        } else {
            let me = room.lobby.seat_of(&room.client);
            let ready =
                me.is_some_and(|n| matches!(&room.lobby.seats[n].who, Who::Person { ready: true, .. }));
            items.push(self.switch("lobby.ready", ready, false));
        }
        if let Some(key) = problem {
            items.push(Item::Lines {
                id: "lobby.problem".into(),
                lines: vec![self.strings.get(key)],
            });
        }
        items.push(self.quiet("lobby.leave", "lobby.leave"));
        Panel::new("match", self.strings.get("lobby.match"), items)
    }

    pub(super) fn lobby_panels(&self, room: &Room) -> Vec<Panel> {
        if room.heard {
            return vec![self.seats_panel(room), self.match_panel(room)];
        }
        vec![Panel::new(
            "finding",
            self.strings.get("lobby.heading"),
            vec![
                Item::Lines {
                    id: "lobby.code".into(),
                    lines: vec![room.code.clone()],
                },
                self.quiet("lobby.leave", "lobby.leave"),
            ],
        )]
    }

    /// The match, until the HUD lands. `state` is what the online check
    /// compares: each of the first two seats' marks, then the newest checksum
    /// with its tick.
    pub(super) fn play_panels(&self, room: &Room) -> Vec<Panel> {
        let mut items = Vec::new();
        if let Some(n) = room.counting(self.now_ms) {
            items.push(Item::Lines {
                id: "play.count".into(),
                lines: vec![self.strings.fill("play.starting", &[("n", n.to_string())])],
            });
        }
        items.push(Item::Lines {
            id: "state".into(),
            lines: vec![state_line(room)],
        });
        items.push(self.quiet("play.leave", "play.leave"));
        vec![Panel::new("play", String::new(), items)]
    }

    /// The winners' screen: who won and why, docked beside the winning team.
    pub(super) fn won_panels(&self, room: &Room) -> Vec<Panel> {
        let Some(outcome) = room.game.as_ref().and_then(Match::outcome) else {
            return Vec::new();
        };
        let (heading, why) = hud::result(&self.strings, outcome);
        let mut won = Panel::new("won", heading, vec![self.button("play.lobby", "play.lobby")]);
        won.sub = why;
        vec![won]
    }
}

fn state_line(room: &Room) -> String {
    let moved = |seat| room.game.as_ref().map_or(0, |game| game.moved(seat));
    let (tick, sum) = room.session.newest_sum().unwrap_or((0, 0));
    format!("{} : {} : {tick} : {sum}", moved(0), moved(1))
}
