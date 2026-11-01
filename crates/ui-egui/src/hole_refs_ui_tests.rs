//! A Single hole placed by distances from reference edges, through the window (#37): pick the
//! face, pick two edges (each distance is measured from the pick), type the distances (one in
//! the dialog, one in its box on the canvas), OK. The hole sits at those distances; Edit Feature
//! picks the edges again with their distances, and moving the hole's place measures them again.
//! A Cylinder standing on a picked face takes reference edges the same way (#6380).

use serde_json::json;
use solidcraft_engine::Sel;
use solidcraft_engine::doc::FeatureKind;
use solidcraft_engine::geom::Vec3;

use crate::dialogs::Kind;
use crate::scenario::Harness;

/// Where a world point is on screen.
fn screen(h: &mut Harness, p: Vec3) -> [f64; 2] {
    h.at(&json!({"world": [p.x, p.y, p.z]})).unwrap()
}

/// A 50 × 40 × 10 plate from the origin, seen from the top.
fn plate() -> Harness {
    let mut h = Harness::new();
    h.app.set_origin_shown(false);
    h.app.run("solid.box", json!({"length": 50, "width": 40, "height": 10, "corner": [0, 0, 0]})).unwrap();
    h.step(&json!({"call": "ui.view", "params": {"view": "top"}})).unwrap();
    h.settle();
    h
}

/// The open Hole dialog's reference distances.
fn distances(h: &Harness) -> [String; 2] {
    match h.app.dialog.as_ref().map(|d| &d.kind) {
        Some(Kind::Hole { opts, .. }) => opts.refs.distance.clone(),
        _ => panic!("the hole dialog is open"),
    }
}

/// Where the open dialog places the hole (its face pick).
fn place(h: &Harness) -> Vec3 {
    match h.app.dialog.as_ref().and_then(|d| d.inputs.first()).and_then(|i| i.items.first()) {
        Some(Sel::Face { point, .. }) => *point,
        other => panic!("a face place, not {other:?}"),
    }
}

/// The edge picked into the open dialog's input `label`.
fn picked_edge(h: &Harness, label: &str) -> Option<Vec3> {
    let d = h.app.dialog.as_ref()?;
    match d.inputs.iter().find(|i| i.label == label)?.items.first()? {
        Sel::Edge { point, .. } => Some(*point),
        _ => None,
    }
}

/// Replace the text of the field at `handle` with `text`.
fn type_into(h: &mut Harness, handle: &str, text: &str) {
    h.step(&json!({"click": {"handle": handle}})).unwrap();
    h.step(&json!({"key": "A", "cmd": true})).unwrap();
    h.step(&json!({"text": text})).unwrap();
    h.frames(2);
}

/// Whether the model holds material at `p`.
fn solid_at(h: &Harness, p: Vec3) -> bool {
    h.app.session.model.state().bodies.iter().any(|b| b.body.contains(p))
}

#[test]
fn a_hole_is_placed_by_its_distances_from_two_edges() {
    let mut h = plate();
    h.step(&json!({"start": "solid.hole"})).unwrap();
    let at = screen(&mut h, Vec3::new(30.0, 25.0, 10.0));
    h.step(&json!({"click": at})).unwrap();
    assert!(place(&h).dist(Vec3::new(30.0, 25.0, 10.0)) < 0.5, "the face is picked where clicked: {:?}", place(&h));
    let picked = place(&h);
    // The reference inputs come after the face; picking an edge measures its distance from the
    // pick, and the second input takes the next edge.
    h.step(&json!({"click": {"handle": "dialog:Reference 1"}})).unwrap();
    let left = screen(&mut h, Vec3::new(0.0, 20.0, 10.0));
    h.step(&json!({"click": left})).unwrap();
    let front = screen(&mut h, Vec3::new(25.0, 0.0, 10.0));
    h.step(&json!({"click": front})).unwrap();
    h.frames(2);
    assert!(picked_edge(&h, "Reference 1").is_some_and(|p| p.x.abs() < 1e-6), "the left edge is Reference 1: {:?}", picked_edge(&h, "Reference 1"));
    assert!(picked_edge(&h, "Reference 2").is_some_and(|p| p.y.abs() < 1e-6), "the front edge is Reference 2: {:?}", picked_edge(&h, "Reference 2"));
    let measured = distances(&h);
    let value = |s: &str| h.app.session.doc.eval(s, solidcraft_engine::doc::expr::Kind::Length).unwrap();
    assert!((value(&measured[0]) - picked.x).abs() < 1e-3 && (value(&measured[1]) - picked.y).abs() < 1e-3, "{measured:?} from {picked:?}");
    // The distances show on the canvas, each with a box to type in.
    assert!(h.at(&json!({"handle": "canvas:Distance 1"})).is_ok() && h.at(&json!({"handle": "canvas:Distance 2"})).is_ok());
    // Typed in the dialog and on the canvas: the hole goes there.
    type_into(&mut h, "dialog:Distance 1", "10 mm");
    type_into(&mut h, "canvas:Distance 2", "12 mm");
    assert_eq!(distances(&h), ["10 mm", "12 mm"]);
    assert!(place(&h).dist(Vec3::new(10.0, 12.0, 10.0)) < 1e-6, "the hole moved to its distances: {:?}", place(&h));
    h.step(&json!({"widget": "OK"})).unwrap();
    assert!(h.app.dialog.is_none(), "OK closed the dialog: {:?}", h.app.dialog.as_ref().map(|d| &d.error));
    let f = h.app.session.doc.features.last().unwrap().clone();
    let FeatureKind::Hole { position, references, .. } = &f.kind else { panic!("the last feature is a hole: {}", f.kind.type_name()) };
    assert!(position.0.dist(Vec3::new(10.0, 12.0, 10.0)) < 1e-6, "{position:?}");
    assert_eq!(references.iter().map(|r| r.distance.to_string()).collect::<Vec<_>>(), ["10 mm", "12 mm"]);
    assert!(h.app.session.model.result(f.id).is_some_and(|r| r.error.is_none()));
    assert!(
        !solid_at(&h, Vec3::new(10.0, 12.0, 8.0)) && solid_at(&h, Vec3::new(30.0, 25.0, 8.0)),
        "drilled at (10, 12), not where the face was picked"
    );

    // Edit Feature: the edges picked again with their distances, the hole where they put it.
    h.step(&json!({"call": "ui.editFeature", "params": {"feature": f.id}})).unwrap();
    h.frames(4);
    assert!(picked_edge(&h, "Reference 1").is_some_and(|p| p.x.abs() < 1e-6), "{:?}", picked_edge(&h, "Reference 1"));
    assert!(picked_edge(&h, "Reference 2").is_some_and(|p| p.y.abs() < 1e-6), "{:?}", picked_edge(&h, "Reference 2"));
    assert_eq!(distances(&h), ["10 mm", "12 mm"]);
    assert!(place(&h).dist(Vec3::new(10.0, 12.0, 10.0)) < 1e-6, "{:?}", place(&h));
    // The hole moved on its face (a new pick, or its handle dragged): the distances follow.
    if let Some(Sel::Face { point, .. }) = h.app.dialog.as_mut().and_then(|d| d.inputs.first_mut()).and_then(|i| i.items.first_mut()) {
        *point = Vec3::new(20.0, 30.0, 10.0);
    }
    h.frames(2);
    assert_eq!(distances(&h), ["20 mm", "30 mm"]);
    h.step(&json!({"widget": "OK"})).unwrap();
    assert!(h.app.dialog.is_none(), "{:?}", h.app.dialog.as_ref().map(|d| &d.error));
    assert!(!solid_at(&h, Vec3::new(20.0, 30.0, 8.0)) && solid_at(&h, Vec3::new(10.0, 12.0, 8.0)), "the edit moved the hole to (20, 30)");
}

/// Reference edges place one hole: with places on two faces, OK says to pick one.
#[test]
fn reference_edges_place_one_hole() {
    let mut h = plate();
    h.step(&json!({"start": "solid.hole"})).unwrap();
    let at = screen(&mut h, Vec3::new(30.0, 25.0, 10.0));
    h.step(&json!({"click": at})).unwrap();
    // A second hole on another face (the bottom), as a pick there would place it.
    let d = h.app.dialog.as_mut().unwrap();
    let Some(Sel::Face { body, index, .. }) = d.inputs[0].items.first().cloned() else { panic!("a face place") };
    d.inputs[0].items.push(Sel::Face { body, index: index + 1, point: Vec3::new(40.0, 10.0, 0.0) });
    h.step(&json!({"click": {"handle": "dialog:Reference 1"}})).unwrap();
    let left = screen(&mut h, Vec3::new(0.0, 20.0, 10.0));
    h.step(&json!({"click": left})).unwrap();
    h.frames(2);
    let d = h.app.dialog.as_ref().unwrap();
    let err = crate::dialogs::apply_commands(&h.app, d).unwrap_err();
    assert!(err.contains("one hole"), "{err}");
}

/// A number typed straight after an edge pick is that edge's distance, not the diameter.
#[test]
fn a_number_typed_after_an_edge_pick_sets_its_distance() {
    let mut h = plate();
    h.step(&json!({"start": "solid.hole"})).unwrap();
    let at = screen(&mut h, Vec3::new(30.0, 25.0, 10.0));
    h.step(&json!({"click": at})).unwrap();
    h.step(&json!({"click": {"handle": "dialog:Reference 1"}})).unwrap();
    let left = screen(&mut h, Vec3::new(0.0, 20.0, 10.0));
    h.step(&json!({"click": left})).unwrap();
    h.frames(2);
    h.step(&json!({"text": "7 mm"})).unwrap();
    h.frames(2);
    assert_eq!(distances(&h)[0], "7 mm");
    assert!((place(&h).x - 7.0).abs() < 1e-6, "the hole is 7 mm from the left edge: {:?}", place(&h));
    let Some(Kind::Hole { diameter, .. }) = h.app.dialog.as_ref().map(|d| &d.kind) else { panic!("the hole dialog") };
    assert_eq!(diameter, "5 mm", "the diameter is left as it was");
}

/// Without value boxes in the view (#9479), the distance's row in the command window takes the
/// number typed after an edge pick, as its box would.
#[test]
fn without_value_boxes_a_number_typed_after_an_edge_pick_sets_its_distance() {
    let mut h = plate();
    h.app.preferences.value_boxes_in_view = false;
    h.step(&json!({"start": "solid.hole"})).unwrap();
    let at = screen(&mut h, Vec3::new(30.0, 25.0, 10.0));
    h.step(&json!({"click": at})).unwrap();
    h.step(&json!({"click": {"handle": "dialog:Reference 1"}})).unwrap();
    let left = screen(&mut h, Vec3::new(0.0, 20.0, 10.0));
    h.step(&json!({"click": left})).unwrap();
    h.frames(2);
    assert!(h.ctx.memory(|m| m.area_rect(egui::Id::new(("sc_hole_ref_area", 0usize)))).is_none(), "a distance box in the view");
    h.step(&json!({"text": "7 mm"})).unwrap();
    h.frames(2);
    assert_eq!(distances(&h)[0], "7 mm");
    assert!((place(&h).x - 7.0).abs() < 1e-6, "the hole is 7 mm from the left edge: {:?}", place(&h));
    let Some(Kind::Hole { diameter, .. }) = h.app.dialog.as_ref().map(|d| &d.kind) else { panic!("the hole dialog") };
    assert_eq!(diameter, "5 mm", "the diameter is left as it was");
}

/// The open Cylinder dialog's reference distances and where it stands.
fn cylinder_dialog(h: &Harness) -> ([String; 2], Vec3) {
    match h.app.dialog.as_ref().map(|d| &d.kind) {
        Some(Kind::Primitive { cmd: "solid.cylinder", refs, at: Some((p, _)), .. }) => (refs.distance.clone(), *p),
        other => panic!("the cylinder dialog is open on a face: {other:?}"),
    }
}

/// With the mouse alone: pick the top face, start Cylinder, pick two edges (each distance
/// measured from the pick), OK. Typing a distance moves the base; Edit Feature picks the edges
/// again.
#[test]
fn a_cylinder_is_placed_by_its_distances_from_two_edges() {
    let mut h = plate();
    let at = screen(&mut h, Vec3::new(30.0, 25.0, 10.0));
    h.step(&json!({"click": at})).unwrap();
    h.step(&json!({"start": "solid.cylinder"})).unwrap();
    h.frames(2);
    let (_, picked) = cylinder_dialog(&h);
    assert!(picked.dist(Vec3::new(30.0, 25.0, 10.0)) < 0.5, "it stands where the face was picked: {picked:?}");
    // Reference 1 takes the first edge picked, Reference 2 the next.
    let left = screen(&mut h, Vec3::new(0.0, 20.0, 10.0));
    h.step(&json!({"click": left})).unwrap();
    let front = screen(&mut h, Vec3::new(25.0, 0.0, 10.0));
    h.step(&json!({"click": front})).unwrap();
    h.frames(2);
    assert!(picked_edge(&h, "Reference 1").is_some_and(|p| p.x.abs() < 1e-6), "{:?}", picked_edge(&h, "Reference 1"));
    assert!(picked_edge(&h, "Reference 2").is_some_and(|p| p.y.abs() < 1e-6), "{:?}", picked_edge(&h, "Reference 2"));
    let (measured, _) = cylinder_dialog(&h);
    let value = |s: &str| h.app.session.doc.eval(s, solidcraft_engine::doc::expr::Kind::Length).unwrap();
    assert!((value(&measured[0]) - picked.x).abs() < 1e-3 && (value(&measured[1]) - picked.y).abs() < 1e-3, "{measured:?} from {picked:?}");
    assert!(h.at(&json!({"handle": "canvas:Distance 1"})).is_ok() && h.at(&json!({"handle": "canvas:Distance 2"})).is_ok());
    h.step(&json!({"widget": "OK"})).unwrap();
    assert!(h.app.dialog.is_none(), "OK closed the dialog: {:?}", h.app.dialog.as_ref().map(|d| &d.error));
    let f = h.app.session.doc.features.last().unwrap().clone();
    let FeatureKind::Cylinder { references, .. } = &f.kind else { panic!("the last feature is a cylinder: {}", f.kind.type_name()) };
    assert_eq!(references.len(), 2);
    assert_eq!(f.edge_names.len(), 2);
    assert!(h.app.session.model.result(f.id).is_some_and(|r| r.error.is_none()));

    // Edit Feature (with the face selected): the edges again with their distances, each input
    // once; a typed distance moves the base.
    let top = crate::dialogs::face_sel(&h.app.session, Vec3::new(45.0, 35.0, 10.0)).unwrap();
    h.app.session.selection = vec![top];
    h.step(&json!({"call": "ui.editFeature", "params": {"feature": f.id}})).unwrap();
    h.frames(4);
    let labels: Vec<&str> = h.app.dialog.as_ref().unwrap().inputs.iter().map(|i| i.label).collect();
    assert_eq!(labels, ["Face", "Reference 1", "Reference 2"]);
    assert!(picked_edge(&h, "Reference 1").is_some_and(|p| p.x.abs() < 1e-6), "{:?}", picked_edge(&h, "Reference 1"));
    assert!(picked_edge(&h, "Reference 2").is_some_and(|p| p.y.abs() < 1e-6), "{:?}", picked_edge(&h, "Reference 2"));
    assert_eq!(cylinder_dialog(&h).0, measured);
    type_into(&mut h, "dialog:Distance 1", "10 mm");
    type_into(&mut h, "canvas:Distance 2", "12 mm");
    assert!(cylinder_dialog(&h).1.dist(Vec3::new(10.0, 12.0, 10.0)) < 1e-6, "{:?}", cylinder_dialog(&h).1);
    h.step(&json!({"widget": "OK"})).unwrap();
    assert!(h.app.dialog.is_none(), "{:?}", h.app.dialog.as_ref().map(|d| &d.error));
    assert!(solid_at(&h, Vec3::new(10.0, 12.0, 12.0)) && !solid_at(&h, Vec3::new(picked.x, picked.y, 12.0)), "the edit moved it to (10, 12)");
}

/// Started with nothing selected, the Cylinder stands at the origin until a face is clicked;
/// then its edges are picked, all with the mouse.
#[test]
fn a_cylinder_started_first_takes_its_face_then_its_edges() {
    let mut h = plate();
    h.step(&json!({"start": "solid.cylinder"})).unwrap();
    h.frames(2);
    let d = h.app.dialog.as_ref().unwrap();
    let labels: Vec<&str> = d.inputs.iter().map(|i| i.label).collect();
    assert_eq!(labels, ["Face", "Reference 1", "Reference 2"]);
    assert!(matches!(d.kind, Kind::Primitive { at: None, .. }), "nothing picked: at the origin");
    let at = screen(&mut h, Vec3::new(30.0, 25.0, 10.0));
    h.step(&json!({"click": at})).unwrap();
    h.frames(2);
    let (_, picked) = cylinder_dialog(&h);
    assert!(picked.dist(Vec3::new(30.0, 25.0, 10.0)) < 0.5, "it stands where the face was clicked: {picked:?}");
    let left = screen(&mut h, Vec3::new(0.0, 20.0, 10.0));
    h.step(&json!({"click": left})).unwrap();
    let front = screen(&mut h, Vec3::new(25.0, 0.0, 10.0));
    h.step(&json!({"click": front})).unwrap();
    h.frames(2);
    assert!(picked_edge(&h, "Reference 1").is_some() && picked_edge(&h, "Reference 2").is_some());
    h.step(&json!({"widget": "OK"})).unwrap();
    assert!(h.app.dialog.is_none(), "{:?}", h.app.dialog.as_ref().map(|d| &d.error));
    let f = h.app.session.doc.features.last().unwrap();
    let FeatureKind::Cylinder { references, .. } = &f.kind else { panic!("the last feature is a cylinder") };
    assert_eq!(references.len(), 2);
}

/// Whether the keyboard is in the value editor of the row whose id is `row` (`ui.diameter`).
fn keyboard_on(h: &Harness, row: &str) -> bool {
    let Some(rect) = crate::scenario::rect_of(&format!("dialog:control:{row}")) else { return false };
    h.ctx.memory(|memory| memory.focused()).and_then(|id| h.ctx.read_response(id)).is_some_and(|response| rect.contains(response.rect.center()))
}

fn pick_active(label: &str) -> bool {
    crate::scenario::count_of(&format!("dialog:selection:{label}:active")) == Some(1.0)
}

/// #10428: Face, Reference 1 and Reference 2 explain themselves, and clicking one then the model
/// fills that field. With value boxes off, the window opens on Diameter; that must not steal the
/// click back.
#[test]
fn cylinder_pick_fields_explain_themselves_and_keep_a_click_in_the_view() {
    let mut h = plate();
    h.app.preferences.value_boxes_in_view = false;
    h.step(&json!({"start": "solid.cylinder"})).unwrap();
    h.frames(3);
    let face = crate::i18n::tr("primitive.face_hint");
    let edge = crate::i18n::tr("primitive.reference_hint");
    assert!(face.contains("existing body") && edge.contains("straight edge"), "{face} / {edge}");
    assert!(face.starts_with(crate::i18n::tr("primitive.face_pick")), "the Face hint starts with the chip: {face}");
    assert!(edge.starts_with(crate::i18n::tr("primitive.edge_pick")), "the reference hint starts with the chip: {edge}");
    assert!(h.widget(face).is_some(), "the open window says what Face is for");
    assert!(h.widget(crate::i18n::tr("primitive.face_pick")).is_some(), "Face says to click a face or vertex");
    assert!(crate::i18n::tr("primitive.face_pick").contains("vertex"), "the chip names every kind the hint names");
    assert!(h.widget(crate::i18n::tr("primitive.edge_pick")).is_some(), "a reference says to click a straight edge");
    assert!(keyboard_on(&h, "ui.diameter"), "the size field starts with the keyboard, as the report shows");
    assert!(!pick_active("Face"), "Diameter holds the keyboard, so Face does not also look active");

    // The name is part of the field: clicking "Reference 1", not only its chip, arms it.
    let label = crate::scenario::rect_of("dialog:label:Reference 1").expect("Reference 1's name").center();
    h.step(&json!({"click": [label.x, label.y]})).unwrap();
    h.frames(2);
    assert!(pick_active("Reference 1"), "Reference 1 is the pick field");
    assert!(!keyboard_on(&h, "ui.diameter"), "arming a pick field takes the keyboard off Diameter");
    assert!(h.widget(edge).is_some(), "the hint follows the field that was clicked");
    // A click on the face, not on an edge, selects nothing and must not jump back to Diameter.
    h.step(&json!({"click": {"world": [30, 25, 10]}})).unwrap();
    h.frames(3);
    assert!(picked_edge(&h, "Reference 1").is_none(), "a face is not an edge");
    assert!(pick_active("Reference 1"), "the field stays armed so the next click can fill it");
    assert!(!keyboard_on(&h, "ui.diameter"), "a miss does not hand the keyboard to Diameter");
    // The edge under the cursor fills Reference 1.
    h.step(&json!({"click": {"world": [0, 20, 10]}})).unwrap();
    h.frames(3);
    assert!(picked_edge(&h, "Reference 1").is_some_and(|p| p.x.abs() < 1e-6), "{:?}", picked_edge(&h, "Reference 1"));
    assert!(!keyboard_on(&h, "ui.diameter"), "the edge click stays with the reference, not Diameter");

    // Face, then the plate: the cylinder stands on that face.
    h.step(&json!({"click": {"handle": "dialog:Face"}})).unwrap();
    h.frames(2);
    assert!(pick_active("Face"));
    h.step(&json!({"click": {"world": [30, 25, 10]}})).unwrap();
    h.frames(3);
    let face_at = h.app.dialog.as_ref().and_then(|d| d.inputs.first()).and_then(|i| i.items.first());
    assert!(matches!(face_at, Some(Sel::Face { .. })), "the face click filled Face, not nothing: {face_at:?}");
    assert!(!keyboard_on(&h, "ui.diameter"), "filling Face does not jump back to Diameter");
}

/// #10428: Box, Sphere, and Torus Point. Clicking the name, then a point on the model, fills the placement.
#[test]
fn point_fields_explain_themselves_and_take_a_click_on_the_model() {
    let mut h = plate();
    h.app.preferences.value_boxes_in_view = false;
    h.app.preferences.auto_operation = false;
    for cmd in ["solid.box", "solid.torus", "solid.sphere"] {
        h.step(&json!({"start": cmd})).unwrap();
        h.frames(3);
        let point_hint = crate::i18n::tr("primitive.point_hint");
        let point_chip = crate::i18n::tr("primitive.point_pick");
        assert!(point_hint.starts_with(point_chip), "{cmd}: hint {point_hint} / chip {point_chip}");
        assert!(h.widget(point_hint).is_some(), "{cmd} explains Point");
        assert!(h.widget(point_chip).is_some(), "{cmd}");
        let size = if cmd == "solid.box" {
            "ui.length"
        } else if cmd == "solid.torus" {
            "ui.major_radius"
        } else {
            "ui.diameter"
        };
        assert!(keyboard_on(&h, size), "{cmd} opens on its first size");
        let label = crate::scenario::rect_of("dialog:label:Point").unwrap_or_else(|| panic!("{cmd} Point")).center();
        h.step(&json!({"click": [label.x, label.y]})).unwrap();
        h.frames(2);
        assert!(pick_active("Point"), "{cmd}");
        assert!(!keyboard_on(&h, size), "{cmd}: Point keeps the keyboard");
        h.step(&json!({"click": {"world": [25, 20, 10]}})).unwrap();
        h.frames(3);
        let point = h.app.dialog.as_ref().and_then(|d| d.extra.get("corner").or_else(|| d.extra.get("center")));
        let coords = point.and_then(serde_json::Value::as_array).cloned().unwrap_or_default();
        let value = |i: usize| {
            coords
                .get(i)
                .and_then(serde_json::Value::as_str)
                .and_then(|t| h.app.session.doc.eval(t, solidcraft_engine::doc::expr::Kind::Length).ok())
                .unwrap_or(f64::NAN)
        };
        assert!((value(0) - 25.0).abs() < 0.5 && (value(1) - 20.0).abs() < 0.5 && (value(2) - 10.0).abs() < 0.5, "{cmd} placement {coords:?}");
        assert!(!keyboard_on(&h, size), "{cmd}: the click did not jump back to the size");
        h.step(&json!({"widget": "Cancel"})).unwrap();
        h.frames(2);
    }
}

/// #10428: Hole still returns the keyboard to a number after a Reference click that misses.
/// Click the face, arm Reference 1, click the face again (not an edge), then type. The 7 is
/// entered. A primitive pick field would have kept the keyboard instead.
#[test]
fn a_missed_hole_reference_click_returns_the_keyboard_to_a_number() {
    let mut h = plate();
    h.app.preferences.value_boxes_in_view = false;
    h.step(&json!({"start": "solid.hole"})).unwrap();
    let at = screen(&mut h, Vec3::new(30.0, 25.0, 10.0));
    h.step(&json!({"click": at})).unwrap();
    h.frames(2);
    h.step(&json!({"click": {"handle": "dialog:Reference 1"}})).unwrap();
    h.frames(2);
    let diameter_before = match h.app.dialog.as_ref().map(|d| &d.kind) {
        Some(Kind::Hole { diameter, .. }) => diameter.clone(),
        _ => panic!("the hole dialog"),
    };
    h.step(&json!({"click": at})).unwrap();
    h.frames(3);
    assert!(picked_edge(&h, "Reference 1").is_none(), "a face is not an edge");
    h.step(&json!({"text": "7"})).unwrap();
    h.frames(2);
    let Some(Kind::Hole { depth, diameter, .. }) = h.app.dialog.as_ref().map(|d| &d.kind) else { panic!("the hole dialog") };
    assert_eq!(depth, "7", "the 7 replaced Depth, the first number field");
    assert_eq!(diameter, &diameter_before, "Diameter is unchanged");
    assert!(picked_edge(&h, "Reference 1").is_none(), "Reference 1 stays empty");
}

/// The hole's Face and Reference rows keep their own wording. The primitive prompts are not theirs.
#[test]
fn a_hole_dialog_does_not_use_the_primitive_pick_wording() {
    let mut h = plate();
    h.step(&json!({"start": "solid.hole"})).unwrap();
    h.frames(3);
    assert!(h.widget(crate::i18n::tr("primitive.face_hint")).is_none());
    assert!(h.widget(crate::i18n::tr("primitive.face_pick")).is_none());
    assert!(h.widget("Face").is_some(), "the hole still names its face row");
}
