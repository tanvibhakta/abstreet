use crate::app::App;
use crate::challenges::Challenge;
use crate::cutscene::CutsceneBuilder;
use crate::edit::EditMode;
use crate::game::{State, Transition};
use crate::sandbox::gameplay::{challenge_header, GameplayMode, GameplayState};
use crate::sandbox::{SandboxControls, SandboxMode};
use ezgui::{
    Btn, Color, Composite, EventCtx, GfxCtx, HorizontalAlignment, Line, Outcome, TextExt,
    VerticalAlignment, Widget,
};
use geom::{Duration, Time};

const THRESHOLD: Duration = Duration::const_seconds(30.0);

pub struct FixTrafficSignals {
    top_center: Composite,
    time: Time,
    failed_at: Option<Time>,
    mode: GameplayMode,
}

impl FixTrafficSignals {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn GameplayState> {
        Box::new(FixTrafficSignals {
            top_center: make_top_center(ctx, app, None),
            time: Time::START_OF_DAY,
            failed_at: None,
            mode: GameplayMode::FixTrafficSignals,
        })
    }

    pub fn cutscene_pt1(ctx: &mut EventCtx, app: &App, _: &GameplayMode) -> Box<dyn State> {
        CutsceneBuilder::new()
            .boss("I hope you've had your coffee. There's a huge mess downtown.")
            .player("Did two buses get tangled together again?")
            .boss("Worse. SCOOT along Mercer is going haywire.")
            .player("SCOOT?")
            .boss(
                "You know, Split Cycle Offset Optimization Technique, the traffic signal \
                 coordination system? Did you sleep through college or what?",
            )
            .boss(
                "It's offline. All the traffic signals look like they've been reset to industry \
                 defaults.",
            )
            .player("Uh oh. Too much scooter traffic overwhelm it? Eh? EHH?")
            .boss("...")
            .boss("You know, not every problem you will face in life is caused by a pun.")
            .boss(
                "Most, in fact, will be caused by me ruining your life because you won't take \
                 your job seriously.",
            )
            .player("Sorry, boss.")
            .boss(
                "Oh no... reports are coming in, ALL of the traffic signals downtown are screwed \
                 up!",
            )
            .boss(
                "You need to go fix all of them. But listen, you haven't got much time. Focus on \
                 the worst problems first.",
            )
            .player("Sigh... it's going to be a long day.")
            .narrator(format!(
                "Don't let the delay for anybody to get through one traffic signal exceed {}",
                THRESHOLD
            ))
            .build(ctx, app)
    }
}

impl GameplayState for FixTrafficSignals {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        _: &mut SandboxControls,
    ) -> Option<Transition> {
        if self.time != app.primary.sim.time() {
            self.time = app.primary.sim.time();
            if self.failed_at.is_none() {
                // TODO We need to check every 5 minutes or force a blocking alert or something.
                let problems = app.primary.sim.delayed_intersections(THRESHOLD);
                if !problems.is_empty() {
                    self.failed_at = Some(app.primary.sim.time());
                    self.top_center = make_top_center(ctx, app, self.failed_at);
                    // TODO warp to problem
                    // TODO popup
                }
            }

            if app.primary.sim.is_done() {
                // TODO win condition
            }
        }

        match self.top_center.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "edit map" => {
                    return Some(Transition::Push(Box::new(EditMode::new(
                        ctx,
                        app,
                        self.mode.clone(),
                    ))));
                }
                "instructions" => {
                    return Some(Transition::Push((Challenge::find(&self.mode)
                        .0
                        .cutscene
                        .unwrap())(
                        ctx, app, &self.mode
                    )));
                }
                "try again" => {
                    app.primary.clear_sim();
                    return Some(Transition::Replace(Box::new(SandboxMode::new(
                        ctx,
                        app,
                        self.mode.clone(),
                    ))));
                }
                _ => unreachable!(),
            },
            None => {}
        }

        None
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.top_center.draw(g);
    }
}

fn make_top_center(ctx: &mut EventCtx, app: &App, failed_at: Option<Time>) -> Composite {
    Composite::new(
        Widget::col(vec![
            challenge_header(ctx, "Fix traffic signals"),
            if let Some(t) = failed_at {
                Widget::row(vec![
                    Line(format!("Delay exceeded {} at {}", THRESHOLD, t))
                        .fg(Color::RED)
                        .draw(ctx)
                        .centered_vert()
                        .margin_right(10),
                    Btn::text_fg("try again").build_def(ctx, None),
                ])
            } else {
                format!("Keep delay under {} ... so far, so good", THRESHOLD).draw_text(ctx)
            },
        ])
        .bg(app.cs.panel_bg)
        .padding(5),
    )
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
    .build(ctx)
}
