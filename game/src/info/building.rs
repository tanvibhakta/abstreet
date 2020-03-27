use crate::app::App;
use crate::helpers::ID;
use crate::info::{header_btns, make_table, make_tabs, Details, Tab};
use ezgui::{Btn, EventCtx, Line, Text, TextExt, Widget};
use geom::Time;
use map_model::BuildingID;
use sim::{TripEndpoint, TripMode, TripResult};

pub fn info(ctx: &mut EventCtx, app: &App, details: &mut Details, id: BuildingID) -> Vec<Widget> {
    let mut rows = header(ctx, app, details, id, Tab::BldgInfo(id));
    let b = app.primary.map.get_b(id);

    let mut kv = Vec::new();

    kv.push(("Address", b.just_address(&app.primary.map)));
    if let Some(name) = b.just_name() {
        kv.push(("Name", name.to_string()));
    }

    if let Some(ref p) = b.parking {
        kv.push(("Parking", format!("{} spots via {}", p.num_stalls, p.name)));
    } else {
        kv.push(("Parking", "None".to_string()));
    }

    rows.extend(make_table(ctx, kv));

    let mut txt = Text::new();

    if !b.amenities.is_empty() {
        txt.add(Line(""));
        if b.amenities.len() > 1 {
            txt.add(Line(format!("{} amenities:", b.amenities.len())));
        }
        for (name, amenity) in &b.amenities {
            txt.add(Line(format!("- {} (a {})", name, amenity)));
        }
    }

    // TODO Rethink this
    let trip_lines = app
        .primary
        .sim
        .count_trips(TripEndpoint::Bldg(id))
        .describe();
    if !trip_lines.is_empty() {
        txt.add(Line(""));
        for line in trip_lines {
            txt.add(Line(line));
        }
    }

    let cars = app.primary.sim.get_parked_cars_by_owner(id);
    if !cars.is_empty() {
        txt.add(Line(""));
        txt.add(Line(format!(
            "{} parked cars owned by this building",
            cars.len()
        )));
        // TODO Jump to it or see status
        for p in cars {
            txt.add(Line(format!("- {}", p.vehicle.id)));
        }
    }

    if !txt.is_empty() {
        rows.push(txt.draw(ctx))
    }

    rows
}

pub fn debug(ctx: &mut EventCtx, app: &App, details: &mut Details, id: BuildingID) -> Vec<Widget> {
    let mut rows = header(ctx, app, details, id, Tab::BldgDebug(id));
    let b = app.primary.map.get_b(id);

    rows.extend(make_table(
        ctx,
        vec![(
            "Dist along sidewalk",
            b.front_path.sidewalk.dist_along().to_string(),
        )],
    ));
    rows.push("Raw OpenStreetMap data".draw_text(ctx));
    rows.extend(make_table(ctx, b.osm_tags.clone().into_iter().collect()));

    rows
}

pub fn people(ctx: &mut EventCtx, app: &App, details: &mut Details, id: BuildingID) -> Vec<Widget> {
    let mut rows = header(ctx, app, details, id, Tab::BldgPeople(id));
    // TODO Sort/group better
    // Show minimal info: ID, next departure time, type of that trip
    for p in app.primary.sim.bldg_to_people(id) {
        let person = app.primary.sim.get_person(p);

        let mut next_trip: Option<(Time, TripMode)> = None;
        for t in &person.trips {
            match app.primary.sim.trip_to_agent(*t) {
                TripResult::TripNotStarted => {
                    let (start_time, _, _, mode) = app.primary.sim.trip_info(*t);
                    next_trip = Some((start_time, mode));
                    break;
                }
                TripResult::Ok(_) | TripResult::ModeChange => {
                    // TODO What to do here? This is meant for building callers right now
                    break;
                }
                TripResult::TripDone => {}
                TripResult::TripDoesntExist => unreachable!(),
            }
        }

        let label = format!("Person #{}", p.0);
        details
            .hyperlinks
            .insert(label.clone(), Tab::PersonStatus(p));
        rows.push(Widget::col(vec![
            Btn::text_bg1(label).build_def(ctx, None),
            if let Some((t, mode)) = next_trip {
                format!("Leaving in {} to {}", t - app.primary.sim.time(), mode).draw_text(ctx)
            } else {
                "Staying inside".draw_text(ctx)
            },
        ]));
    }

    rows
}

fn header(
    ctx: &EventCtx,
    app: &App,
    details: &mut Details,
    id: BuildingID,
    tab: Tab,
) -> Vec<Widget> {
    let mut rows = vec![];

    rows.push(Widget::row(vec![
        Line(format!("Building #{}", id.0))
            .small_heading()
            .draw(ctx),
        header_btns(ctx),
    ]));

    rows.push(make_tabs(
        ctx,
        &mut details.hyperlinks,
        tab,
        vec![
            ("Info", Tab::BldgInfo(id)),
            ("Debug", Tab::BldgDebug(id)),
            ("People", Tab::BldgPeople(id)),
        ],
    ));

    // TODO On every tab?
    for p in app.primary.sim.get_parked_cars_by_owner(id) {
        let shape = app
            .primary
            .draw_map
            .get_obj(
                ID::Car(p.vehicle.id),
                app,
                &mut app.primary.draw_map.agents.borrow_mut(),
                ctx.prerender,
            )
            .unwrap()
            .get_outline(&app.primary.map);
        details.unzoomed.push(
            app.cs.get("something associated with something else"),
            shape.clone(),
        );
        details.zoomed.push(
            app.cs.get("something associated with something else"),
            shape,
        );
    }

    rows
}