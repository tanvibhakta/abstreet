// TODO pub so challenges can grab cutscenes. Weird?
pub mod commute;
mod create_gridlock;
pub mod fix_traffic_signals;
mod freeform;
mod play_scenario;
pub mod spawner;
mod tutorial;

pub use self::tutorial::{Tutorial, TutorialPointer, TutorialState};
use crate::app::App;
use crate::common::ContextualActions;
use crate::game::Transition;
use crate::helpers::ID;
use crate::sandbox::SandboxControls;
use abstutil::Timer;
use ezgui::{lctrl, Btn, Color, EventCtx, GeomBatch, GfxCtx, Key, Line, Widget};
use geom::{Duration, Polygon};
use map_model::{EditCmd, EditIntersection, Map, MapEdits};
use rand_xorshift::XorShiftRng;
use sim::{Analytics, OrigPersonID, Scenario, ScenarioGenerator};

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum GameplayMode {
    // TODO Maybe this should be "sandbox"
    // Map path
    Freeform(String),
    // Map path, scenario name
    PlayScenario(String, String),
    // Map path
    CreateGridlock(String),
    FixTrafficSignals,
    OptimizeCommute(OrigPersonID, Duration),

    // current
    Tutorial(TutorialPointer),
}

pub trait GameplayState: downcast_rs::Downcast {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        controls: &mut SandboxControls,
    ) -> Option<Transition>;
    fn draw(&self, g: &mut GfxCtx, app: &App);

    fn can_move_canvas(&self) -> bool {
        true
    }
    fn can_examine_objects(&self) -> bool {
        true
    }
    fn has_common(&self) -> bool {
        true
    }
    fn has_tool_panel(&self) -> bool {
        true
    }
    fn has_time_panel(&self) -> bool {
        true
    }
    fn has_speed(&self) -> bool {
        true
    }
    fn has_agent_meter(&self) -> bool {
        true
    }
    fn has_minimap(&self) -> bool {
        true
    }
}
downcast_rs::impl_downcast!(GameplayState);

impl GameplayMode {
    pub fn map_path(&self) -> String {
        match self {
            GameplayMode::Freeform(ref path) => path.to_string(),
            GameplayMode::PlayScenario(ref path, _) => path.to_string(),
            GameplayMode::CreateGridlock(ref path) => path.to_string(),
            // TODO tmp
            GameplayMode::FixTrafficSignals => abstutil::path_map("montlake"),
            GameplayMode::OptimizeCommute(_, _) => abstutil::path_map("montlake"),
            GameplayMode::Tutorial(_) => abstutil::path_map("montlake"),
        }
    }

    pub fn scenario(
        &self,
        map: &Map,
        num_agents: Option<usize>,
        mut rng: XorShiftRng,
        timer: &mut Timer,
    ) -> Option<Scenario> {
        let name = match self {
            GameplayMode::Freeform(_) => {
                return None;
            }
            GameplayMode::PlayScenario(_, ref scenario) => scenario.to_string(),
            // TODO Some of these WILL have scenarios!
            GameplayMode::Tutorial(_) => {
                return None;
            }
            _ => "weekday".to_string(),
        };
        Some(if name == "random" {
            (if let Some(n) = num_agents {
                ScenarioGenerator::scaled_run(n)
            } else {
                ScenarioGenerator::small_run(map)
            })
            .generate(map, &mut rng, &mut Timer::new("generate scenario"))
        } else if name == "just buses" {
            let mut s = Scenario::empty(map, "just buses");
            s.only_seed_buses = None;
            s
        } else if name == "5 weekdays repeated" {
            let s: Scenario =
                abstutil::read_binary(abstutil::path_scenario(map.get_name(), "weekday"), timer);
            s.repeat_days(5)
        } else {
            let path = abstutil::path_scenario(map.get_name(), &name);
            match abstutil::maybe_read_binary(path.clone(), timer) {
                Ok(s) => s,
                Err(err) => {
                    println!("\n\n{} is missing or corrupt. Check https://github.com/dabreegster/abstreet/blob/master/docs/dev.md and file an issue if you have trouble.", path);
                    println!("\n{}", err);
                    std::process::exit(1);
                }
            }
        })
    }

    pub fn can_edit_lanes(&self) -> bool {
        match self {
            GameplayMode::FixTrafficSignals => false,
            _ => true,
        }
    }

    pub fn can_edit_stop_signs(&self) -> bool {
        match self {
            GameplayMode::FixTrafficSignals => false,
            _ => true,
        }
    }

    pub fn allows(&self, edits: &MapEdits) -> bool {
        for cmd in &edits.commands {
            match cmd {
                EditCmd::ChangeLaneType { .. }
                | EditCmd::ReverseLane { .. }
                | EditCmd::ChangeSpeedLimit { .. } => {
                    if !self.can_edit_lanes() {
                        return false;
                    }
                }
                EditCmd::ChangeIntersection { ref new, .. } => match new {
                    EditIntersection::StopSign(_) => {
                        if !self.can_edit_stop_signs() {
                            return false;
                        }
                    }
                    _ => {}
                },
            }
        }
        true
    }

    pub fn initialize(&self, app: &mut App, ctx: &mut EventCtx) -> Box<dyn GameplayState> {
        ctx.loading_screen("setup challenge", |ctx, timer| {
            if &abstutil::basename(&self.map_path()) != app.primary.map.get_name() {
                app.switch_map(ctx, self.map_path());
            }

            if let Some(scenario) = self.scenario(
                &app.primary.map,
                app.primary.current_flags.num_agents,
                app.primary.current_flags.sim_flags.make_rng(),
                timer,
            ) {
                scenario.instantiate(
                    &mut app.primary.sim,
                    &app.primary.map,
                    &mut app.primary.current_flags.sim_flags.make_rng(),
                    timer,
                );
                app.primary
                    .sim
                    .normal_step(&app.primary.map, Duration::seconds(0.1));

                // Maybe we've already got prebaked data for this map+scenario.
                if !app
                    .has_prebaked()
                    .map(|(m, s)| m == &scenario.map_name && s == &scenario.scenario_name)
                    .unwrap_or(false)
                {
                    // If there's no prebaked data, so be it; some functionality disappears
                    if let Ok(prebaked) = abstutil::maybe_read_binary::<Analytics>(
                        abstutil::path_prebaked_results(
                            &scenario.map_name,
                            &scenario.scenario_name,
                        ),
                        timer,
                    ) {
                        app.set_prebaked(Some((
                            scenario.map_name.clone(),
                            scenario.scenario_name.clone(),
                            prebaked,
                        )));
                    } else {
                        println!(
                            "WARNING: Missing or corrupt prebaked results for {} on {}, some \
                             stuff might break",
                            scenario.scenario_name, scenario.map_name
                        );
                        app.set_prebaked(None);
                    }
                }
            }
        });
        match self {
            GameplayMode::Freeform(_) => freeform::Freeform::new(ctx, app, self.clone()),
            GameplayMode::PlayScenario(_, ref scenario) => {
                play_scenario::PlayScenario::new(ctx, app, scenario, self.clone())
            }
            GameplayMode::CreateGridlock(_) => {
                create_gridlock::CreateGridlock::new(ctx, app, self.clone())
            }
            GameplayMode::FixTrafficSignals => {
                fix_traffic_signals::FixTrafficSignals::new(ctx, app)
            }
            GameplayMode::OptimizeCommute(p, goal) => {
                commute::OptimizeCommute::new(ctx, app, *p, *goal)
            }
            GameplayMode::Tutorial(current) => Tutorial::new(ctx, app, *current),
        }
    }
}

impl ContextualActions for GameplayMode {
    fn actions(&self, app: &App, id: ID) -> Vec<(Key, String)> {
        match self {
            GameplayMode::Freeform(_) => spawner::actions(app, id),
            GameplayMode::Tutorial(_) => tutorial::actions(app, id),
            _ => Vec::new(),
        }
    }

    fn execute(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        id: ID,
        action: String,
        _: &mut bool,
    ) -> Transition {
        match self {
            GameplayMode::Freeform(_) => spawner::execute(ctx, app, id, action),
            GameplayMode::Tutorial(_) => tutorial::execute(ctx, app, id, action),
            _ => unreachable!(),
        }
    }

    fn is_paused(&self) -> bool {
        unreachable!()
    }
}

fn challenge_header(ctx: &mut EventCtx, title: &str) -> Widget {
    Widget::row(vec![
        Line(title)
            .small_heading()
            .draw(ctx)
            .centered_vert()
            .margin_right(10),
        Btn::svg_def("../data/system/assets/tools/info.svg")
            .build(ctx, "instructions", None)
            .centered_vert()
            .margin_right(10),
        Widget::draw_batch(
            ctx,
            GeomBatch::from(vec![(Color::WHITE, Polygon::rectangle(2.0, 50.0))]),
        )
        .margin_right(10),
        Btn::svg_def("../data/system/assets/tools/edit_map.svg")
            .build(ctx, "edit map", lctrl(Key::E))
            .centered_vert(),
    ])
    .padding(5)
}
