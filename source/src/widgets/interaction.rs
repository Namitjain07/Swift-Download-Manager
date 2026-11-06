//! Tests that use the widgets the way a person does: the pointer moves, presses and releases, keys
//! go down and up, and text is typed. They check what happens, not just where things are drawn.

use super::*;
use crate::theme::testing::{click, key, run, themed};
use crate::theme::ThemeMode;
use eframe::egui::{self, vec2, Event, Id, Key, Pos2, Rect, Ui};
use std::cell::Cell;

const SIZE: egui::Vec2 = vec2(900.0, 700.0);

fn ctx() -> egui::Context {
    themed(ThemeMode::Light)
}

#[test]
fn clicking_a_button_presses_it_and_clicking_beside_it_does_not() {
    let ctx = ctx();
    let rect = Cell::new(Rect::NOTHING);
    let clicks = Cell::new(0);
    let mut add = |ui: &mut Ui| {
        ui.horizontal(|ui| {
            let r = button(ui, ButtonKind::Primary, "Save");
            rect.set(r.rect);
            if r.clicked() {
                clicks.set(clicks.get() + 1);
            }
        });
    };
    run(&ctx, SIZE, vec![], &mut add);
    click(&ctx, SIZE, rect.get().center(), &mut add);
    assert_eq!(clicks.get(), 1, "a click in the middle of the button");
    click(&ctx, SIZE, rect.get().left_top() + vec2(4.0, 4.0), &mut add);
    assert_eq!(clicks.get(), 2, "a click near its corner still counts: the whole capsule is the target");
    click(&ctx, SIZE, rect.get().right_bottom() + vec2(60.0, 60.0), &mut add);
    assert_eq!(clicks.get(), 2, "a click away from it does not");
}

#[test]
fn a_disabled_button_cannot_be_clicked() {
    let ctx = ctx();
    let rect = Cell::new(Rect::NOTHING);
    let clicks = Cell::new(0);
    let mut add = |ui: &mut Ui| {
        ui.add_enabled_ui(false, |ui| {
            let r = button(ui, ButtonKind::Primary, "Find links");
            rect.set(r.rect);
            if r.clicked() {
                clicks.set(clicks.get() + 1);
            }
        });
    };
    run(&ctx, SIZE, vec![], &mut add);
    click(&ctx, SIZE, rect.get().center(), &mut add);
    assert_eq!(clicks.get(), 0);
}

#[test]
fn a_switch_flips_on_each_click() {
    let ctx = ctx();
    let rect = Cell::new(Rect::NOTHING);
    let on = Cell::new(false);
    let mut add = |ui: &mut Ui| {
        let mut v = on.get();
        rect.set(switch(ui, &mut v).rect);
        on.set(v);
    };
    run(&ctx, SIZE, vec![], &mut add);
    click(&ctx, SIZE, rect.get().center(), &mut add);
    assert!(on.get());
    click(&ctx, SIZE, rect.get().center(), &mut add);
    assert!(!on.get());
}

#[test]
fn the_stepper_counts_up_and_down_and_stops_at_its_limits() {
    let ctx = ctx();
    let rect = Cell::new(Rect::NOTHING);
    let value = Cell::new(8u32);
    let mut add = |ui: &mut Ui| {
        let mut v = value.get();
        rect.set(stepper(ui, &mut v, 1..=10).rect);
        value.set(v);
    };
    run(&ctx, SIZE, vec![], &mut add);
    let plus = Pos2::new(rect.get().right() - 2.0 - 18.0, rect.get().center().y);
    let minus = Pos2::new(rect.get().left() + 2.0 + 18.0, rect.get().center().y);
    click(&ctx, SIZE, plus, &mut add);
    assert_eq!(value.get(), 9);
    click(&ctx, SIZE, plus, &mut add);
    click(&ctx, SIZE, plus, &mut add);
    assert_eq!(value.get(), 10, "it stops at the top of the range");
    click(&ctx, SIZE, minus, &mut add);
    assert_eq!(value.get(), 9);
    value.set(1);
    click(&ctx, SIZE, minus, &mut add);
    assert_eq!(value.get(), 1, "and at the bottom");
}

#[test]
fn the_segmented_control_picks_the_segment_that_was_clicked() {
    let ctx = ctx();
    let rect = Cell::new(Rect::NOTHING);
    let value = Cell::new(0u8);
    let mut add = |ui: &mut Ui| {
        let mut v = value.get();
        rect.set(segmented(ui, &[(0u8, "Automatic"), (1, "Light"), (2, "Dark")], &mut v).rect);
        value.set(v);
    };
    run(&ctx, SIZE, vec![], &mut add);
    let seg_w = (rect.get().width() - 8.0) / 3.0;
    for want in [2u8, 1, 0, 2] {
        let x = rect.get().left() + 4.0 + seg_w * (want as f32 + 0.5);
        click(&ctx, SIZE, Pos2::new(x, rect.get().center().y), &mut add);
        assert_eq!(value.get(), want);
    }
}

#[test]
fn a_tick_row_toggles_when_its_label_is_clicked() {
    let ctx = ctx();
    let rect = Cell::new(Rect::NOTHING);
    let on = Cell::new(false);
    let mut add = |ui: &mut Ui| {
        let mut v = on.get();
        rect.set(check_row(ui, &mut v, "Remember my choice").rect);
        on.set(v);
    };
    run(&ctx, SIZE, vec![], &mut add);
    // Far from the circle, on the text: the whole line is the target.
    click(&ctx, SIZE, Pos2::new(rect.get().left() + 120.0, rect.get().center().y), &mut add);
    assert!(on.get());
}

#[test]
fn menu_rows_click_unless_they_are_disabled() {
    let ctx = ctx();
    let (enabled, disabled) = (Cell::new(Rect::NOTHING), Cell::new(Rect::NOTHING));
    let hits = Cell::new((0, 0));
    let mut add = |ui: &mut Ui| {
        menu_width(ui);
        let a = menu_item(ui, "Add new download…", "Ctrl+N", true);
        let b = menu_item(ui, "Open file", "", false);
        enabled.set(a.rect);
        disabled.set(b.rect);
        let (x, y) = hits.get();
        hits.set((x + a.clicked() as i32, y + b.clicked() as i32));
    };
    run(&ctx, SIZE, vec![], &mut add);
    click(&ctx, SIZE, enabled.get().center(), &mut add);
    click(&ctx, SIZE, disabled.get().center(), &mut add);
    assert_eq!(hits.get(), (1, 0));
}

#[test]
fn text_typed_into_a_focused_field_arrives_and_letters_do_not_reach_a_number_field() {
    let ctx = ctx();
    let rect = Cell::new(Rect::NOTHING);
    let text = std::cell::RefCell::new(String::new());
    let mut add = |ui: &mut Ui| {
        let mut t = text.borrow_mut();
        rect.set(ui.add(field(&mut t).desired_width(300.0)).rect);
    };
    run(&ctx, SIZE, vec![], &mut add);
    click(&ctx, SIZE, rect.get().center(), &mut add);
    run(&ctx, SIZE, vec![Event::Text("https://x.org/a.zip".into())], &mut add);
    assert_eq!(*text.borrow(), "https://x.org/a.zip");

    // A number field takes digits only and keeps its value inside the range.
    let value = Cell::new(512u64);
    let nrect = Cell::new(Rect::NOTHING);
    let mut add = |ui: &mut Ui| {
        let mut v = value.get();
        nrect.set(number_field(ui, &mut v, 1..=100_000, "KB/s", 150.0).rect);
        value.set(v);
    };
    run(&ctx, SIZE, vec![], &mut add);
    click(&ctx, SIZE, nrect.get().center(), &mut add);
    run(&ctx, SIZE, vec![Event::Text("x".into())], &mut add);
    assert_eq!(value.get(), 512, "a letter changes nothing");
    run(&ctx, SIZE, vec![Event::Text("9".into())], &mut add);
    assert_eq!(value.get(), 5129);
    run(&ctx, SIZE, vec![Event::Text("99999999".into())], &mut add);
    assert_eq!(value.get(), 100_000, "and too big a number is held at the top of the range");
}

#[test]
fn a_drop_down_opens_on_a_click_and_picks_the_option_that_is_clicked() {
    let ctx = ctx();
    let rect = Cell::new(Rect::NOTHING);
    let value = Cell::new("Main Queue".to_string());
    let options: Vec<(String, String)> = ["Main Queue", "Night", "Weekend"].iter().map(|s| (s.to_string(), s.to_string())).collect();
    let mut add = |ui: &mut Ui| {
        let mut v = value.take();
        rect.set(dropdown(ui, "q", 240.0, &options, &mut v).rect);
        value.set(v);
    };
    run(&ctx, SIZE, vec![], &mut add);
    let field = rect.get();
    click(&ctx, SIZE, field.center(), &mut add);
    // The list hangs under the field: an 8 px margin, then rows 38 high with 2 between.
    let row = |k: usize| Pos2::new(field.left() + 30.0, field.bottom() + 8.0 + 19.0 + 40.0 * k as f32);
    click(&ctx, SIZE, row(1), &mut add);
    assert_eq!(value.take(), "Night");
}

#[test]
fn a_sheet_closes_with_its_x_with_escape_and_not_by_a_stray_click_on_the_backdrop() {
    let ctx = ctx();
    let closed = Cell::new(false);
    let mut add = |ui: &mut Ui| {
        let out = sheet("t", "Scheduler").width(440.0).show(ui.ctx(), |s| {
            s.body(|ui| {
                ui.label("body");
            });
            s.footer(|ui| {
                primary_button(ui, "Save");
            });
        });
        if out.close {
            closed.set(true);
        }
    };
    for _ in 0..3 {
        run(&ctx, SIZE, vec![], &mut add);
    }
    let area = ctx.memory(|m| m.area_rect(Id::unique(("sdm_sheet", "t")))).expect("the sheet is shown");
    assert!(!closed.get());

    // A click on the dimmed page behind the sheet does not close it: a half-filled form must not be lost.
    click(&ctx, SIZE, Pos2::new(5.0, 5.0), &mut add);
    assert!(!closed.get(), "the backdrop must not close a form");

    // Escape does.
    key(&ctx, SIZE, Key::Escape, &mut add);
    assert!(closed.get(), "Escape closes the sheet");

    // So does the x in the header.
    closed.set(false);
    for _ in 0..3 {
        run(&ctx, SIZE, vec![], &mut add);
    }
    let x = Pos2::new(area.right() - 14.0 - 18.0, area.top() + 32.0);
    click(&ctx, SIZE, x, &mut add);
    assert!(closed.get(), "the close button closes the sheet");
}

#[test]
fn an_information_sheet_closes_on_a_click_outside_it() {
    let ctx = ctx();
    let closed = Cell::new(false);
    let mut add = |ui: &mut Ui| {
        let out = sheet("about", "About").width(420.0).backdrop_closes().show(ui.ctx(), |s| {
            s.body(|ui| {
                ui.label("Swift Download Manager");
            });
        });
        if out.close {
            closed.set(true);
        }
    };
    for _ in 0..3 {
        run(&ctx, SIZE, vec![], &mut add);
    }
    click(&ctx, SIZE, Pos2::new(5.0, 5.0), &mut add);
    assert!(closed.get());
}

#[test]
fn clicking_inside_a_sheet_does_not_reach_the_page_behind_it() {
    let ctx = ctx();
    let page_clicks = Cell::new(0);
    let page_rect = Cell::new(Rect::NOTHING);
    let sheet_rect = Cell::new(Rect::NOTHING);
    let mut add = |ui: &mut Ui| {
        // A button on the page, exactly where the sheet will cover it.
        ui.set_min_size(SIZE);
        let r = ui.put(Rect::from_center_size(Pos2::new(450.0, 350.0), vec2(120.0, 40.0)), egui::Button::new("page"));
        page_rect.set(r.rect);
        if r.clicked() {
            page_clicks.set(page_clicks.get() + 1);
        }
        sheet("t", "Options").width(500.0).show(ui.ctx(), |s| {
            s.body(|ui| {
                ui.allocate_space(vec2(100.0, 200.0));
            });
        });
        if let Some(a) = ui.ctx().memory(|m| m.area_rect(Id::unique(("sdm_sheet", "t")))) {
            sheet_rect.set(a);
        }
    };
    for _ in 0..3 {
        run(&ctx, SIZE, vec![], &mut add);
    }
    assert!(sheet_rect.get().contains_rect(page_rect.get()), "the sheet covers the page button");
    click(&ctx, SIZE, page_rect.get().center(), &mut add);
    assert_eq!(page_clicks.get(), 0, "the modal sheet takes the click");
}
