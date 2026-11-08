use super::*;
use crate::lobby::{Lobby, Role};
use crate::online::room::CODE_LEN;
use crate::online::{encode, Msg, PROTOCOL};
use crate::strings::tests::sample;

fn notes() -> String {
    format!(r#"[{{"version": "{VERSION}", "date": "2026-11-06", "notes": ["A."]}}]"#)
}
const GLOSSARY: &str = "### Fold\n\nWhere the sheep go.\n";

fn app() -> App {
    App::new(&sample(), &notes(), GLOSSARY).unwrap()
}

/// Every control id on the screen, in order, by walking the panels' JSON.
fn ids(app: &App) -> Vec<String> {
    let v = serde_json::to_value(app.ui()).unwrap();
    let mut out = Vec::new();
    for panel in v["panels"].as_array().unwrap() {
        for item in panel["items"].as_array().unwrap() {
            if let Some(id) = item["id"].as_str() {
                out.push(id.to_string());
            }
        }
    }
    out
}

#[test]
fn the_title_offers_solo_online_and_settings_and_the_version_opens_whats_new() {
    let mut a = app();
    assert_eq!(a.ui().screen, "title");
    assert_eq!(ids(&a), ["menu.solo", "menu.online", "menu.settings"]);
    assert_eq!(a.ui().version, VERSION);
    a.act("version", "").unwrap();
    assert_eq!(a.ui().screen, "notes");
    a.act("menu.back", "").unwrap();
    assert_eq!(a.ui().screen, "title");
}

#[test]
fn the_build_refuses_patch_notes_that_do_not_start_at_its_version() {
    let old = r#"[{"version": "0.0.9", "date": "2026-11-06", "notes": ["A."]}]"#;
    assert_eq!(
        App::new(&sample(), old, "").unwrap_err(),
        format!("The patch notes start at 0.0.9, and this build is {VERSION}.")
    );
}

#[test]
fn settings_reach_the_save_and_a_reload_brings_them_back() {
    let mut a = app();
    a.act("settings.volume", "35").unwrap();
    a.act("settings.dog_mode", "on").unwrap();
    let text = a.ui().save;
    let mut b = app();
    assert_eq!(b.ui().volume, save::DEFAULT_VOLUME);
    b.act("save.load", &text).unwrap();
    assert_eq!(b.ui().volume, 35);
    assert!(b.ui().dog_mode);
}

#[test]
fn a_sound_level_past_the_top_stops_at_the_top_and_junk_changes_nothing() {
    let mut a = app();
    a.act("settings.volume", "400").unwrap();
    assert_eq!(a.ui().volume, 100);
    assert_eq!(
        a.act("settings.volume", "loud").unwrap_err(),
        "\"loud\" is not a sound level."
    );
    assert_eq!(a.ui().volume, 100);
    assert_eq!(
        a.act("settings.dog_mode", "maybe").unwrap_err(),
        "\"maybe\" is neither on nor off."
    );
}

#[test]
fn a_refused_save_is_kept_for_the_page_and_settings_offers_it_back() {
    let mut a = app();
    a.act("save.load", "{nope").unwrap();
    assert_eq!(a.ui().keep_refused, "{nope");
    a.act("save.kept", "").unwrap();
    assert_eq!(a.ui().keep_refused, "");
    a.act("menu.settings", "").unwrap();
    assert!(ids(&a).contains(&"save.old_file".to_string()));
}

#[test]
fn an_unknown_action_is_refused_by_name() {
    assert_eq!(
        app().act("screen.jump", "x").unwrap_err(),
        "Unknown action \"screen.jump\"."
    );
}

/// An app with the flags the page sends as it loads, and no transport.
fn flagged() -> App {
    let mut a = app();
    let flags = [
        ("flag.seed", "42"),
        ("flag.client", "me"),
        ("flag.build", "b"),
        ("flag.page_url", "here/"),
        ("flag.net_off", ""),
        ("flag.fast", ""),
        ("view.size", "1920 1080"),
    ];
    for (name, value) in flags {
        a.act(name, value).unwrap();
    }
    a
}

/// `flagged`, in a room hosted by Morag, whose roster has reached this browser.
fn joined() -> App {
    let mut a = flagged();
    a.act("flag.room", "ABCDEF").unwrap();
    a.net_event("connected", "h", "", true, 0);
    let hello = encode(&Msg::Hello {
        protocol: PROTOCOL,
        build: "b".into(),
    });
    a.net_event("message", "h", &hello, false, 0);
    let mut lobby = Lobby::new("h", "Morag");
    lobby.add_person("me", "");
    a.net_event("message", "h", &encode(&Msg::Roster { lobby }), false, 0);
    a
}

fn panels_text(a: &App) -> String {
    serde_json::to_string(&a.ui().panels).unwrap()
}

#[test]
fn solo_opens_a_lobby_that_starts_a_match_and_leaving_goes_home() {
    let mut a = flagged();
    a.act("menu.solo", "").unwrap();
    assert_eq!(a.ui().screen, "lobby");
    assert!(a.ui().live, "the view follows the room each frame");
    assert_eq!(a.ui().hash, "", "a solo room has no address");
    a.act("lobby.start", "").unwrap();
    a.pump(17, 0);
    let ui = a.ui();
    assert_eq!(ui.screen, "play");
    assert!(ui.capture, "keys go to the core");
    assert!(!ui.version_opens, "no What's New in a match");
    assert_eq!(ui.keys.get("ArrowRight"), Some(&crate::game::KEY_RIGHT));
    a.act("play.leave", "").unwrap();
    assert_eq!(a.ui().screen, "title");
    assert!(!a.ui().live);
    assert!(
        a.pump(34, 0).requests.is_empty(),
        "a solo room never opened the transport"
    );
}

#[test]
fn the_lobby_docks_beside_the_open_middle_and_the_lineup_marks_this_browser() {
    let mut a = flagged();
    a.act("menu.solo", "").unwrap();
    let ui = a.ui();
    assert_eq!(
        ui.dock,
        Dock::Sides {
            panel: 400,
            open_x: 448,
            open_w: 1024
        }
    );
    assert_eq!(ui.lineup.len(), 2, "this browser and the bot Wolf");
    assert!(ui.lineup[0].me);
    assert!(!ui.lineup[1].me);
    assert_eq!(ui.lineup[1].look, "wolf");
    a.act("seat.2.bot", "").unwrap();
    assert_eq!(a.ui().lineup.len(), 3);
    a.act("lobby.leave", "").unwrap();
    assert_eq!(a.ui().dock, Dock::Center);
    assert!(a.ui().lineup.is_empty());
}

#[test]
fn a_hosted_room_shows_its_invite_link_and_the_address_carries_its_code() {
    let mut a = flagged();
    a.act("online.host", "").unwrap();
    let hash = a.ui().hash;
    let code = hash.strip_prefix("#room=").expect("the address names the room");
    assert_eq!(code.len(), CODE_LEN);
    assert!(panels_text(&a).contains(&format!("here/#room={code}")));
}

#[test]
fn an_invite_link_joins_its_room_and_a_bad_one_opens_online_and_says_why() {
    let mut a = flagged();
    a.act("flag.room", "abcdef").unwrap();
    assert_eq!(a.ui().screen, "lobby");
    assert_eq!(a.ui().hash, "#room=ABCDEF");
    assert_eq!(
        a.ui().dock,
        Dock::Center,
        "nothing to dock until the host answers"
    );

    let mut a = flagged();
    a.act("flag.room", "no").unwrap();
    assert_eq!(a.ui().screen, "online");
    assert_eq!(a.ui().status, a.strings.get("online.bad_code"));
}

#[test]
fn a_typed_code_joins_and_a_wrong_one_stays_to_be_fixed() {
    let mut a = flagged();
    a.act("menu.online", "").unwrap();
    a.act("online.code", "nope").unwrap();
    a.act("online.join", "").unwrap();
    assert_eq!(a.ui().screen, "online");
    assert_eq!(a.ui().status, a.strings.get("online.bad_code"));
    assert!(panels_text(&a).contains("nope"), "the field keeps what was typed");
    a.act("online.code", "abcdef").unwrap();
    assert_eq!(a.ui().status, "", "editing the code clears the complaint");
    a.act("online.join", "").unwrap();
    assert_eq!(a.ui().screen, "lobby");
}

#[test]
fn leaving_a_room_tells_the_transport_once() {
    let mut a = flagged();
    a.act("online.host", "").unwrap();
    a.act("lobby.leave", "").unwrap();
    assert_eq!(a.ui().hash, "");
    assert_eq!(a.pump(1, 0).requests, vec![Request::Leave]);
    assert!(a.pump(2, 0).requests.is_empty());
}

#[test]
fn a_joiner_sees_the_settings_locked_and_a_ready_button_where_the_host_has_start() {
    let a = joined();
    let shown = ids(&a);
    assert!(shown.contains(&"lobby.ready".to_string()), "{shown:?}");
    assert!(!shown.contains(&"lobby.start".to_string()), "{shown:?}");
    let v = serde_json::to_value(a.ui().panels).unwrap();
    let sheep = v[1]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["id"] == "lobby.sheep")
        .unwrap();
    assert_eq!(sheep["disabled"], true);
    let text = panels_text(&a);
    assert!(text.contains("Morag"));
    assert!(text.contains(&a.strings.fill("seat.default_name", &[("n", "2".to_string())])));
    assert!(!text.contains("seat.0.bot"), "a joiner adds no bots");
}

#[test]
fn the_lobby_offers_three_map_sizes_filled_by_defaults_and_locked_for_a_joiner() {
    let a = joined();
    let v = serde_json::to_value(a.ui().panels).unwrap();
    let items = v[1]["items"].as_array().unwrap();
    let size = items.iter().find(|i| i["id"] == "lobby.map_size").unwrap();
    assert_eq!(size["label"], a.strings.get("lobby.map_size"));
    // two players: Defaults picks Small
    assert_eq!(size["value"], "small");
    assert_eq!(size["disabled"], true);
    let words: Vec<String> = size["options"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["text"].as_str().unwrap().to_string())
        .collect();
    let sizes = ["map_size.small", "map_size.medium", "map_size.big"];
    assert_eq!(words, sizes.map(|key| a.strings.get(key)));
}

/// The joiner's lobby with its own seat marked ready or not.
fn joined_ready(ready: bool) -> App {
    let mut a = joined();
    let mut lobby = Lobby::new("h", "Morag");
    lobby.add_person("me", "");
    lobby.set_ready("me", ready);
    a.net_event("message", "h", &encode(&Msg::Roster { lobby }), false, 0);
    a
}

/// The items of the panel with this id, as the page gets them.
fn items_of(a: &App, panel: &str) -> Vec<serde_json::Value> {
    let v = serde_json::to_value(a.ui().panels).unwrap();
    let found = v.as_array().unwrap().iter().find(|p| p["kind"] == panel).unwrap();
    found["items"].as_array().unwrap().clone()
}

#[test]
fn the_ready_button_ends_the_match_panel_and_says_what_it_does() {
    for (ready, key) in [(false, "lobby.ready"), (true, "lobby.not_ready")] {
        let a = joined_ready(ready);
        let items = items_of(&a, "match");
        let last = items.last().unwrap();
        assert_eq!(
            (&last["type"], &last["id"]),
            (&"button".into(), &"lobby.ready".into())
        );
        assert_eq!(last["text"], a.strings.get(key), "ready: {ready}");
        assert!(last.get("quiet").is_none_or(|q| q == false), "a real button");
    }
}

#[test]
fn the_name_field_sits_under_your_own_seat() {
    let a = joined();
    let ids: Vec<String> = items_of(&a, "seats")
        .iter()
        .map(|i| i["id"].as_str().unwrap_or_default().to_string())
        .collect();
    let mine = ids.iter().position(|id| id == "seat.1").unwrap();
    assert_eq!(ids[mine + 1], "lobby.name", "{ids:?}");
    assert!(!items_of(&a, "match").iter().any(|i| i["id"] == "lobby.name"));
}

#[test]
fn sheep_and_eats_show_unknown_while_defaults_waits_on_a_random_seat() {
    let shown = |a: &App, id: &str| {
        let items = items_of(a, "match");
        let item = items.iter().find(|i| i["id"] == id).unwrap();
        item.get("text").cloned()
    };
    let a = joined();
    let unknown = serde_json::Value::from(a.strings.get("lobby.unknown"));
    assert_eq!(
        shown(&a, "lobby.sheep"),
        Some(unknown.clone()),
        "both seats Random"
    );
    assert_eq!(shown(&a, "lobby.eats"), Some(unknown));
    let mut sure = joined();
    let mut lobby = Lobby::new("h", "Morag");
    lobby.add_person("me", "");
    lobby.set_role(0, Role::Wolf);
    lobby.set_role(1, Role::Sheepdog);
    sure.net_event("message", "h", &encode(&Msg::Roster { lobby }), false, 0);
    assert_eq!(shown(&sure, "lobby.sheep"), None, "the numbers show");
}

#[test]
fn a_name_typed_in_a_lobby_is_saved_and_opens_the_next_room() {
    let mut a = flagged();
    a.act("online.host", "").unwrap();
    a.act("lobby.name", "  Morag ").unwrap();
    assert_eq!(a.save.name, "Morag");
    a.act("lobby.leave", "").unwrap();
    a.act("online.host", "").unwrap();
    assert!(panels_text(&a).contains("Morag"));
}

#[test]
fn the_old_file_button_has_the_page_offer_the_refused_save_once() {
    let mut a = app();
    a.act("save.old_file", "").unwrap();
    assert!(a.ui().offer_old_file);
    a.act("save.offered", "").unwrap();
    assert!(!a.ui().offer_old_file);
}

#[test]
fn the_window_size_is_two_numbers_and_anything_else_is_refused() {
    let mut a = app();
    a.act("view.size", "800 600").unwrap();
    assert_eq!((a.width, a.height), (800, 600));
    for junk in ["800", "wide tall", ""] {
        assert!(a.act("view.size", junk).is_err(), "{junk:?}");
    }
    assert_eq!((a.width, a.height), (800, 600));
}

/// The `disabled` of the control `id` on the screen.
fn disabled(app: &App, id: &str) -> bool {
    let v = serde_json::to_value(app.ui()).unwrap();
    let panels = v["panels"].as_array().unwrap();
    let item = panels
        .iter()
        .flat_map(|p| p["items"].as_array().unwrap())
        .find(|i| i["id"] == id)
        .unwrap_or_else(|| panic!("{id} is not on the screen"));
    item["disabled"].as_bool().unwrap_or(false)
}

#[test]
fn online_offers_pasted_codes_only_under_the_codes_flag() {
    let mut a = app();
    a.act("menu.online", "").unwrap();
    assert!(!ids(&a).contains(&"online.paste_host".to_string()));
    assert!(!ids(&a).contains(&"online.paste_join".to_string()));
    let mut a = app();
    a.act("flag.codes", "").unwrap();
    a.act("menu.online", "").unwrap();
    let on_screen = ids(&a);
    assert!(on_screen.contains(&"online.paste_host".to_string()));
    assert!(on_screen.contains(&"online.paste_join".to_string()));
}

#[test]
fn a_host_by_code_opens_a_lobby_with_no_room_code_and_shows_the_invite_when_it_comes() {
    let mut a = app();
    a.act("online.paste_host", "").unwrap();
    assert_eq!(a.ui().screen, "lobby");
    assert_eq!(a.ui().hash, "");
    assert!(!ids(&a).contains(&"online.offer_out".to_string()));
    assert!(!ids(&a).contains(&"lobby.copy_code".to_string()));
    assert!(disabled(&a, "online.accept_answer"));
    a.net_event("code", "", "Oinvite", false, 1);
    assert!(panels_text(&a).contains("Oinvite"));
    a.act("online.answer_in", "Areply").unwrap();
    assert!(!disabled(&a, "online.accept_answer"));
    a.act("online.accept_answer", "").unwrap();
    assert_eq!(a.typed_reply, "");
    let asked = Request::AcceptReply {
        reply: "Areply".to_string(),
    };
    assert!(a.pump(2, 0).requests.contains(&asked));
}

#[test]
fn a_joiner_makes_a_reply_on_the_form_and_goes_to_the_lobby_when_the_host_is_heard() {
    let mut a = app();
    a.act("flag.client", "j").unwrap();
    a.act("online.paste_join", "").unwrap();
    assert_eq!(a.ui().screen, "paste");
    assert!(disabled(&a, "online.make_answer"));
    a.act("online.offer_in", "Oinvite").unwrap();
    a.act("online.make_answer", "").unwrap();
    assert_eq!(a.ui().screen, "paste");
    assert!(disabled(&a, "online.make_answer"), "the reply is on its way");
    let asked = Request::JoinByCode {
        invite: "Oinvite".to_string(),
    };
    assert!(a.pump(1, 0).requests.contains(&asked));
    a.net_event("code", "", "Areply", false, 2);
    assert!(panels_text(&a).contains("Areply"));
    a.net_event("connected", "h", "", true, 3);
    let host = Lobby::new("h", "Hosty");
    let hello = encode(&Msg::Hello {
        protocol: PROTOCOL,
        build: String::new(),
    });
    a.net_event("message", "h", &hello, false, 4);
    a.net_event("message", "h", &encode(&Msg::Roster { lobby: host }), false, 5);
    a.pump(6, 0);
    assert_eq!(a.ui().screen, "lobby");
}

#[test]
fn back_from_the_form_leaves_its_room_and_forgets_the_paste() {
    let mut a = app();
    a.act("online.paste_join", "").unwrap();
    a.act("online.offer_in", "Oinvite").unwrap();
    a.act("online.make_answer", "").unwrap();
    a.act("online.paste_back", "").unwrap();
    assert_eq!(a.ui().screen, "online");
    assert!(a.room.is_none());
    assert_eq!(a.typed_invite, "");
    assert!(a.pump(1, 0).requests.contains(&Request::Leave));
}

/// A solo match, this browser a Sheepdog against a bot Wolf, under way.
fn solo_match() -> App {
    let mut a = flagged();
    a.act("menu.solo", "").unwrap();
    a.act("lobby.start", "").unwrap();
    a.pump(17, 0);
    a
}

/// Ends the match on its next tick, the Wolves' way or the Sheepdogs'.
fn end_it(a: &mut App, wolves: bool, now_ms: i64) {
    a.act("flag.end", if wolves { "wolf" } else { "sheepdog" })
        .unwrap();
    a.pump(now_ms, 0);
    assert!(a.room.as_ref().unwrap().over());
}

#[test]
fn the_last_moment_holds_and_then_the_scene_gives_way_to_the_winners() {
    let mut a = solo_match();
    a.fast = false;
    end_it(&mut a, false, 1000);
    let held = a.ui();
    assert_eq!(held.screen, "play");
    assert!(!held.scene.is_empty(), "the match stays on screen for the hold");
    assert!(!held.capture);
    a.pump(1000 + WIN_HOLD_MS - 1, 0);
    assert_eq!(a.ui().screen, "play");
    a.pump(1000 + WIN_HOLD_MS, 0);
    let won = a.ui();
    assert_eq!(won.screen, "won");
    assert_eq!(won.scene, "", "the match's scene closes");
    assert!(won.cheer);
    assert!(!won.version_opens);
    assert_eq!(won.dock, dock::lobby(a.width, a.height));
}

#[test]
fn the_winners_are_the_seats_of_the_team_that_won_and_nobody_else() {
    let mut a = solo_match();
    end_it(&mut a, false, 1000);
    a.pump(1017, 0);
    let dogs = a.ui().lineup;
    assert_eq!(dogs.len(), 1);
    assert_eq!(
        (dogs[0].seat, dogs[0].look.as_str(), dogs[0].me),
        (0, "sheepdog", true)
    );

    let mut a = solo_match();
    end_it(&mut a, true, 1000);
    a.pump(1017, 0);
    let wolves = a.ui().lineup;
    assert_eq!(wolves.len(), 1, "the bot wins too, and this browser's seat lost");
    assert_eq!(
        (wolves[0].seat, wolves[0].look.as_str(), wolves[0].me),
        (1, "wolf", false)
    );
    assert_eq!(ids(&a), ["play.lobby"]);
}

#[test]
fn back_from_the_winners_goes_to_the_lobby_and_the_cheering_stops() {
    let mut a = solo_match();
    end_it(&mut a, false, 1000);
    a.pump(1017, 0);
    assert_eq!(a.ui().screen, "won");
    a.act("play.lobby", "").unwrap();
    a.pump(1034, 0);
    let ui = a.ui();
    assert_eq!(ui.screen, "lobby");
    assert!(!ui.cheer);
    assert_eq!(ui.lineup.len(), 2, "both seats again");
}

#[test]
fn a_new_match_pulls_a_browser_off_the_winners_screen() {
    let mut a = solo_match();
    end_it(&mut a, false, 1000);
    a.pump(1017, 0);
    let room = a.room.as_mut().unwrap();
    // what a joiner's room does on the host's next Start: a match not ended
    let fresh = room.start.clone().unwrap();
    room.begin(fresh);
    a.pump(1034, 0);
    assert_eq!(a.ui().screen, "play");
    assert!(!a.ui().cheer);
}
