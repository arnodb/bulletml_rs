use indextree::{Arena, Node, NodeId};
use std::collections::HashSet;
use std::ops::{Deref, DerefMut};

use crate::tree::{
    BulletML, BulletMLExpression, BulletMLNode, BulletMLType, DirectionType, HVType, SpeedType,
};

/// Set of data required during a BulletML run.
///
/// `D` is the type of the application data used in the [AppRunner](trait.AppRunner.html) callbacks.
pub struct RunnerData<'a, D: 'a> {
    pub bml: &'a BulletML,
    pub data: &'a mut D,
}

type Parameters = Vec<f64>;

/// State information that can be used to call
/// [Runner::new_from_state](struct.Runner.html#method.new_from_state) or
/// [Runner::init_from_state](struct.Runner.html#method.init_from_state) when creating new bullets.
///
/// See also [AppRunner::create_bullet](trait.AppRunner.html#tymethod.create_bullet).
pub struct State {
    bml_type: Option<BulletMLType>,
    nodes: Box<[NodeId]>,
    parameters: Parameters,
}

/// Elementary bullet runner. It is used either to run one single bullet or to run one or more "top"
/// actions.
pub struct Runner<R> {
    runners: Vec<RunnerImpl>,
    app_runner: R,
}

impl<R> Runner<R> {
    /// Creates a new runner for all the "top" actions of the provided BulletML document.
    ///
    /// `app_runner` is the application runner which contains all the specific behaviours.
    ///
    /// `bml` is the parsed BulletML document to be used by the runner until the bullet dies.
    pub fn new(app_runner: R, bml: &BulletML) -> Self {
        let bml_type = bml.get_type();
        let runners = bml
            .root
            .children(&bml.arena)
            .filter(|child| {
                let child_node = &bml.arena[*child];
                child_node.get().is_top_action()
            })
            .map(|action| {
                let state = State {
                    bml_type,
                    nodes: Box::new([action]),
                    parameters: Vec::new(),
                };
                RunnerImpl::new(state)
            })
            .collect();
        Runner {
            runners,
            app_runner,
        }
    }

    /// Reuses this runner for all the "top" actions of the provided BulletML document. It works
    /// the same way as [new](#method.new).
    ///
    /// `app_runner` is the application runner which contains all the specific behaviours.
    ///
    /// `bml` is the parsed BulletML document to be used by the runner until the bullet dies.
    pub fn init<D>(&mut self, bml: &BulletML)
    where
        R: AppRunner<D>,
    {
        let bml_type = bml.get_type();
        self.runners.clear();
        for action in bml.root.children(&bml.arena).filter(|child| {
            let child_node = &bml.arena[*child];
            child_node.get().is_top_action()
        }) {
            let state = State {
                bml_type,
                nodes: Box::new([action]),
                parameters: Vec::new(),
            };
            self.runners.push(RunnerImpl::new(state))
        }
        self.app_runner.init();
    }

    /// Creates a new runner from an existing state.
    ///
    /// `app_runner` is the application runner which contains all the specific behaviours.
    ///
    /// `state` is the state with which
    /// [AppRunner::create_bullet](trait.AppRunner.html#tymethod.create_bullet) is called.
    pub fn new_from_state(app_runner: R, state: State) -> Self {
        Runner {
            runners: vec![RunnerImpl::new(state)],
            app_runner,
        }
    }

    /// Reuses this runner from an existing state.  It works
    /// the same way as [new_from_state](#method.new_from_state) except that the application
    /// runner cannot change.
    ///
    /// `state` is the state with which
    /// [AppRunner::create_bullet](trait.AppRunner.html#tymethod.create_bullet) is called.
    pub fn init_from_state<D>(&mut self, state: State)
    where
        R: AppRunner<D>,
    {
        self.runners.clear();
        self.runners.push(RunnerImpl::new(state));
        self.app_runner.init();
    }

    /// Runs one iteration of this runner.
    ///
    /// `data` contains the application data used in the [AppRunner](trait.AppRunner.html) callbacks.
    pub fn run<D>(&mut self, data: &mut RunnerData<D>)
    where
        R: AppRunner<D>,
    {
        for runner in &mut self.runners {
            runner.run(data, &mut self.app_runner);
        }
    }

    /// Checks whether this runner is alive.
    pub fn is_end(&self) -> bool {
        for runner in &self.runners {
            if runner.is_end() {
                return true;
            }
        }
        false
    }
}

impl<R: Default> Default for Runner<R> {
    fn default() -> Self {
        Runner {
            runners: Vec::default(),
            app_runner: R::default(),
        }
    }
}

impl<R> Deref for Runner<R> {
    type Target = R;
    fn deref(&self) -> &Self::Target {
        &self.app_runner
    }
}

impl<R> DerefMut for Runner<R> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.app_runner
    }
}

/// Application specific BulletML runner trait.
pub trait AppRunner<D> {
    /// Initializes the runner.
    ///
    /// This function is called when a new [Runner](struct.Runner.html) is created/reused.
    fn init(&mut self) {}
    /// Gets this bullet's direction based on application data.
    fn get_bullet_direction(&self, data: &D) -> f64;
    /// Gets this bullet's aim direction based on application data.
    ///
    /// The "target" related to the "aim" notion is application specific.
    fn get_aim_direction(&self, data: &D) -> f64;
    /// Gets this bullet's speed based on application data.
    fn get_bullet_speed(&self, data: &D) -> f64;
    /// Gets the bullet default speed.
    fn get_default_speed(&self) -> f64;
    /// Gets the BulletML "rank", a value between 0 and 1 indicating the level of difficulty.
    /// The value is used in arithmetic expressions with `$rank`.
    fn get_rank(&self, data: &D) -> f64;
    /// Tells the application to create a bullet with the given `direction` and `speed`.
    ///
    /// The simple use case is to create a bullet whose direction and speed won't change until it
    /// disappears or hits the target.
    ///
    /// Nevertheless there could be more complex use cases which involve creating a new runner with
    /// the same BulletML document or even another one.
    fn create_simple_bullet(&mut self, data: &mut D, direction: f64, speed: f64);
    /// Tells the application to create a bullet based on the given `state`, initial `direction`
    /// and initial `speed`.
    ///
    /// The typical use case is to create a new runner with the same BulletML document. See
    /// [Runner::new_from_state](struct.Runner.html#method.new_from_state) and
    /// [Runner::init_from_state](struct.Runner.html#method.init_from_state).
    fn create_bullet(&mut self, data: &mut D, state: State, direction: f64, speed: f64);
    /// Gets the current iteration number.
    fn get_turn(&self, data: &D) -> u32;
    /// Tells the application to make this bullet vanish.
    fn do_vanish(&mut self, data: &mut D);
    fn do_change_direction(&mut self, _data: &mut D, _direction: f64) {}
    /// Tells the application to make this bullet change speed.
    fn do_change_speed(&mut self, _data: &mut D, _speed: f64) {}
    /// Tells the application to make this bullet accelerate.
    fn do_accel_x(&mut self, _: f64) {}
    /// Tells the application to make this bullet accelerate.
    fn do_accel_y(&mut self, _: f64) {}
    /// Gets this bullet's X speed.
    fn get_bullet_speed_x(&self) -> f64 {
        0.
    }
    /// Gets this bullet's Y speed.
    fn get_bullet_speed_y(&self) -> f64 {
        0.
    }
    /// Gets a new random value. The random number generator is managed by the application.
    fn get_rand(&self, data: &mut D) -> f64;
    #[cfg(test)]
    fn log(&mut self, _data: &mut D, _node: &BulletMLNode) {}
}

struct Validatable<T: Copy> {
    value: T,
    valid: bool,
}

impl<T: Copy> Validatable<T> {
    fn get(&self) -> T {
        self.value
    }

    fn is_valid(&self) -> bool {
        self.valid
    }

    fn set(&mut self, value: T) {
        self.value = value;
        self.valid = true;
    }

    fn invalidate(&mut self) {
        self.valid = false;
    }
}

impl<T: Copy + Default> Default for Validatable<T> {
    fn default() -> Self {
        Validatable {
            value: T::default(),
            valid: false,
        }
    }
}

struct LinearFunc<X, Y> {
    first_x: X,
    last_x: X,
    first_y: Y,
    last_y: Y,
    gradient: Y,
}

impl<X, Y> LinearFunc<X, Y>
where
    X: Copy + PartialOrd + std::ops::Sub<Output = X> + Into<Y>,
    Y: Copy
        + Default
        + std::ops::Add<Output = Y>
        + std::ops::Sub<Output = Y>
        + std::ops::Mul<Output = Y>
        + std::ops::Div<Output = Y>,
{
    fn new(first_x: X, last_x: X, first_y: Y, last_y: Y) -> Self {
        Self {
            first_x,
            last_x,
            first_y,
            last_y,
            gradient: (last_y - first_y) / (last_x - first_x).into(),
        }
    }

    fn get_value(&self, x: X) -> Y {
        self.first_y + self.gradient * (x - self.first_x).into()
    }

    fn is_last(&self, x: X) -> bool {
        x >= self.last_x
    }

    fn get_last(&self) -> Y {
        self.last_y
    }
}

struct StackedRef {
    ref_id: NodeId,
    prev: NodeId,
    prev_parameters: Parameters,
}

pub struct RunnerImpl {
    bml_type: Option<BulletMLType>,
    nodes: Box<[NodeId]>,
    root_nodes: HashSet<NodeId>,
    change_dir: Option<LinearFunc<u32, f64>>,
    change_spd: Option<LinearFunc<u32, f64>>,
    accel_x: Option<LinearFunc<u32, f64>>,
    accel_y: Option<LinearFunc<u32, f64>>,
    spd: Validatable<f64>,
    prev_spd: Validatable<f64>,
    dir: Validatable<f64>,
    prev_dir: Validatable<f64>,
    act: Option<NodeId>,
    act_turn: Option<u32>,
    end_turn: u32,
    act_iter: usize,
    end: bool,
    parameters: Parameters,
    repeat_stack: Vec<RepeatElem>,
    ref_stack: Vec<StackedRef>,
}

impl RunnerImpl {
    fn new(state: State) -> Self {
        let act = Some(state.nodes[0]);
        let mut root_nodes = HashSet::new();
        for node in state.nodes.iter() {
            root_nodes.insert(*node);
        }
        RunnerImpl {
            bml_type: state.bml_type,
            nodes: state.nodes,
            root_nodes,
            change_dir: None,
            change_spd: None,
            accel_x: None,
            accel_y: None,
            spd: Validatable::default(),
            prev_spd: Validatable::default(),
            dir: Validatable::default(),
            prev_dir: Validatable::default(),
            act,
            act_turn: None,
            end_turn: 0,
            act_iter: 0,
            end: false,
            parameters: state.parameters,
            repeat_stack: Vec::new(),
            ref_stack: Vec::new(),
        }
    }

    fn run<D>(&mut self, data: &mut RunnerData<D>, runner: &mut dyn AppRunner<D>) {
        if self.is_end() {
            return;
        }
        self.changes(data, runner);
        self.end_turn = runner.get_turn(data.data);
        if self.act.is_none() {
            if !self.is_turn_end()
                && self.change_dir.is_none()
                && self.change_spd.is_none()
                && self.accel_x.is_none()
                && self.accel_y.is_none()
            {
                self.end = true;
            }
            return;
        }
        self.act = Some(self.nodes[self.act_iter]);
        if self.act_turn.is_none() {
            self.act_turn = Some(runner.get_turn(data.data));
        }
        self.run_sub(data, runner);
        match self.act {
            None => {
                self.act_iter += 1;
                if self.act_iter < self.nodes.len() {
                    self.act = Some(self.nodes[self.act_iter]);
                }
            }
            Some(act) => self.nodes[self.act_iter] = act,
        }
    }

    fn is_end(&self) -> bool {
        self.end
    }

    fn is_turn_end(&self) -> bool {
        self.is_end() || self.act_turn.unwrap_or(0) > self.end_turn
    }

    fn do_wait(&mut self, frame: u32) {
        if frame > 0 {
            self.act_turn = Some(self.act_turn.unwrap() + frame);
        }
    }

    fn changes<D>(&mut self, data: &mut RunnerData<D>, runner: &mut dyn AppRunner<D>) {
        let now = runner.get_turn(data.data);
        let reset = if let Some(change_dir) = &self.change_dir {
            if change_dir.is_last(now) {
                runner.do_change_direction(data.data, change_dir.get_last());
                true
            } else {
                runner.do_change_direction(data.data, change_dir.get_value(now));
                false
            }
        } else {
            false
        };
        if reset {
            self.change_dir = None;
        }
        let reset = if let Some(change_spd) = &self.change_spd {
            if change_spd.is_last(now) {
                runner.do_change_speed(data.data, change_spd.get_last());
                true
            } else {
                runner.do_change_speed(data.data, change_spd.get_value(now));
                false
            }
        } else {
            false
        };
        if reset {
            self.change_spd = None;
        }
        let reset = if let Some(accel_x) = &self.accel_x {
            if accel_x.is_last(now) {
                runner.do_accel_x(accel_x.get_last());
                true
            } else {
                runner.do_accel_x(accel_x.get_value(now));
                false
            }
        } else {
            false
        };
        if reset {
            self.accel_x = None;
        }
        let reset = if let Some(accel_y) = &self.accel_y {
            if accel_y.is_last(now) {
                runner.do_accel_y(accel_y.get_last());
                true
            } else {
                runner.do_accel_y(accel_y.get_value(now));
                false
            }
        } else {
            false
        };
        if reset {
            self.accel_y = None;
        }
    }

    fn run_sub<D>(&mut self, data: &mut RunnerData<D>, runner: &mut dyn AppRunner<D>) {
        let bml = data.bml;
        while let Some(act) = self.act {
            if self.is_turn_end() {
                break;
            }
            let mut prev = act;
            let mut prev_node = &bml.arena[act];
            let node = &bml.arena[act];
            #[cfg(test)]
            runner.log(&mut data.data, node.get());
            match node.get() {
                BulletMLNode::Bullet { .. } => self.run_bullet(data, runner),
                BulletMLNode::Action { .. } => self.run_action(node),
                BulletMLNode::Fire { .. } => self.run_fire(data, runner),
                BulletMLNode::ChangeDirection => self.run_change_direction(data, runner),
                BulletMLNode::ChangeSpeed => self.run_change_speed(data, runner),
                BulletMLNode::Accel => self.run_accel(data, runner),
                BulletMLNode::Wait(expr) => self.run_wait(*expr, data, runner),
                BulletMLNode::Repeat => self.run_repeat(act, data, runner),
                BulletMLNode::BulletRef(label) => {
                    self.run_ref(act, bml.bullet_refs[label], data, runner)
                }
                BulletMLNode::ActionRef(label) => {
                    self.run_ref(act, bml.action_refs[label], data, runner)
                }
                BulletMLNode::FireRef(label) => {
                    self.run_ref(act, bml.fire_refs[label], data, runner)
                }
                BulletMLNode::Vanish => self.run_vanish(data, runner),
                _ => (),
            }
            loop {
                if self.act.is_none() {
                    // Unstack reference if needed.
                    if self
                        .ref_stack
                        .last()
                        .map_or(false, |stacked| stacked.ref_id == prev)
                    {
                        let top = self.ref_stack.pop().unwrap();
                        prev = top.prev;
                        prev_node = &bml.arena[prev];
                        self.parameters = top.prev_parameters;
                    }

                    // Jump to next sibling if any.
                    if !self.root_nodes.contains(&prev) {
                        self.act = prev_node.next_sibling();
                    }
                }

                // Found something to run or hit a root node, break.
                if self.act.is_some() || self.root_nodes.contains(&prev) {
                    break;
                }

                // Go to parent unless it is an unfinished Repeat.
                let parent = prev_node.parent();
                let (new_act, new_act_node) = if let Some(parent) = parent {
                    let parent_node = &bml.arena[parent];
                    if let BulletMLNode::Repeat = parent_node.get() {
                        let rep = self.repeat_stack.last_mut().unwrap();
                        rep.iter += 1;
                        if rep.iter < rep.end {
                            // Unfinished Repeat, set act and break loop.
                            self.act = Some(rep.act);
                            break;
                        }
                        // Finished Repeat, pop.
                        self.repeat_stack.pop();
                    }
                    (parent, parent_node)
                } else {
                    panic!("A run node must have a parent");
                };

                prev = new_act;
                prev_node = new_act_node;
            }
        }
    }

    fn get_first_child_id_matching<M, N>(
        arena: &Arena<BulletMLNode>,
        parent: NodeId,
        m: M,
    ) -> Option<NodeId>
    where
        M: Fn(&BulletMLNode) -> Option<N>,
    {
        for child in parent.children(arena) {
            let child_node = &arena[child];
            if m(child_node.get()).is_some() {
                return Some(child);
            }
        }
        None
    }

    fn get_first_child_matching<M, N>(
        arena: &Arena<BulletMLNode>,
        parent: NodeId,
        m: M,
    ) -> Option<N>
    where
        M: Fn(&BulletMLNode) -> Option<N>,
    {
        for child in parent.children(arena) {
            let child_node = &arena[child];
            let n = m(child_node.get());
            if n.is_some() {
                return n;
            }
        }
        None
    }

    fn get_children_ids_matching<M, N>(
        arena: &Arena<BulletMLNode>,
        parent: NodeId,
        m: M,
    ) -> Vec<NodeId>
    where
        M: Fn(&BulletMLNode) -> Option<N>,
    {
        parent
            .children(arena)
            .filter(|child| {
                let child_node = &arena[*child];
                m(child_node.get()).is_some()
            })
            .collect()
    }

    fn shot_init(&mut self) {
        self.spd.invalidate();
        self.dir.invalidate();
    }

    fn get_direction<D>(
        &mut self,
        dir_type: Option<DirectionType>,
        expr: BulletMLExpression,
        data: &mut RunnerData<D>,
        runner: &dyn AppRunner<D>,
    ) -> f64 {
        let direction = self.get_number_contents(expr, data, runner);
        let (mut direction, aim) = match dir_type {
            None => (direction, true),
            Some(DirectionType::Aim) => (direction, true),
            Some(DirectionType::Absolute) => (
                if self.bml_type == Some(BulletMLType::Horizontal) {
                    direction - 90.
                } else {
                    direction
                },
                false,
            ),
            Some(DirectionType::Relative) => {
                (direction + runner.get_bullet_direction(data.data), false)
            }
            Some(DirectionType::Sequence) => {
                if !self.prev_dir.is_valid() {
                    (0., true)
                } else {
                    (direction + self.prev_dir.get(), false)
                }
            }
        };
        if aim {
            direction += runner.get_aim_direction(data.data);
        }
        while direction > 360. {
            direction -= 360.
        }
        while direction < 0. {
            direction += 360.
        }
        self.prev_dir.set(direction);
        direction
    }

    fn set_direction<D>(&mut self, data: &mut RunnerData<D>, runner: &dyn AppRunner<D>) {
        if let Some(act) = self.act {
            let direction =
                Self::get_first_child_matching(&data.bml.arena, act, BulletMLNode::match_direction);
            if let Some((dir_type, dir)) = direction {
                let direction = self.get_direction(dir_type, dir, data, runner);
                self.dir.set(direction);
            }
        }
    }

    fn get_speed<D>(
        &mut self,
        spd_type: Option<SpeedType>,
        expr: BulletMLExpression,
        data: &mut RunnerData<D>,
        runner: &dyn AppRunner<D>,
    ) -> f64 {
        let mut speed = self.get_number_contents(expr, data, runner);
        speed = match spd_type {
            None => speed,
            Some(SpeedType::Absolute) => speed,
            Some(SpeedType::Relative) => speed + runner.get_bullet_speed(data.data),
            Some(SpeedType::Sequence) => {
                if !self.prev_spd.is_valid() {
                    1.
                } else {
                    speed + self.prev_spd.get()
                }
            }
        };
        self.prev_spd.set(speed);
        speed
    }

    fn set_speed<D>(&mut self, data: &mut RunnerData<D>, runner: &dyn AppRunner<D>) {
        if let Some(act) = self.act {
            let speed =
                Self::get_first_child_matching(&data.bml.arena, act, BulletMLNode::match_speed);
            if let Some((spd_type, spd)) = speed {
                let speed = self.get_speed(spd_type, spd, data, runner);
                self.spd.set(speed);
            }
        }
    }

    fn run_bullet<D>(&mut self, data: &mut RunnerData<D>, runner: &mut dyn AppRunner<D>) {
        let arena = &data.bml.arena;
        self.set_speed(data, runner);
        self.set_direction(data, runner);
        if !self.spd.is_valid() {
            let default = runner.get_default_speed();
            self.spd.set(default);
            self.prev_spd.set(default);
        }
        if !self.dir.is_valid() {
            let default = runner.get_aim_direction(data.data);
            self.dir.set(default);
            self.prev_dir.set(default);
        }
        let all_actions = self.act.map_or_else(Vec::new, |act| {
            Self::get_children_ids_matching(arena, act, BulletMLNode::match_any_action)
        });
        if all_actions.is_empty() {
            runner.create_simple_bullet(data.data, self.dir.get(), self.spd.get());
        } else {
            let state = State {
                bml_type: self.bml_type,
                nodes: all_actions.into_boxed_slice(),
                parameters: self.parameters.clone(),
            };
            runner.create_bullet(data.data, state, self.dir.get(), self.spd.get());
        }
        self.act = None;
    }

    fn run_fire<D>(&mut self, data: &mut RunnerData<D>, runner: &dyn AppRunner<D>) {
        self.shot_init();
        self.set_speed(data, runner);
        self.set_direction(data, runner);
        if let Some(act) = self.act {
            let arena = &data.bml.arena;
            let bullet =
                Self::get_first_child_id_matching(arena, act, BulletMLNode::match_any_bullet);
            if bullet.is_some() {
                self.act = bullet;
            }
        }
    }

    fn run_action(&mut self, node: &Node<BulletMLNode>) {
        self.act = node.first_child();
    }

    fn run_wait<D>(
        &mut self,
        expr: BulletMLExpression,
        data: &mut RunnerData<D>,
        runner: &dyn AppRunner<D>,
    ) {
        let frame = self.get_number_contents(expr, data, runner);
        self.do_wait(frame as u32);
        self.act = None;
    }

    fn run_repeat<D>(&mut self, act: NodeId, data: &mut RunnerData<D>, runner: &dyn AppRunner<D>) {
        let times = Self::get_first_child_matching(&data.bml.arena, act, BulletMLNode::match_times);
        if let Some(times) = times {
            let times = self.get_number_contents(times, data, runner) as usize;
            let arena = &data.bml.arena;
            let action =
                Self::get_first_child_id_matching(arena, act, BulletMLNode::match_any_action);
            self.repeat_stack.push(RepeatElem {
                iter: 0,
                end: times,
                act: action.unwrap(),
            });
            self.act = action;
        }
    }

    fn run_ref<D>(
        &mut self,
        act: NodeId,
        ref_id: NodeId,
        data: &mut RunnerData<D>,
        runner: &dyn AppRunner<D>,
    ) {
        let new_parameters = self.get_parameters(data, runner);
        let prev_parameters = std::mem::replace(&mut self.parameters, new_parameters);
        self.ref_stack.push(StackedRef {
            ref_id,
            prev: act,
            prev_parameters,
        });
        self.act = Some(ref_id);
    }

    fn run_change_direction<D>(&mut self, data: &mut RunnerData<D>, runner: &dyn AppRunner<D>) {
        if let Some(act) = self.act {
            let arena = &data.bml.arena;
            let term = Self::get_first_child_matching(arena, act, BulletMLNode::match_term);
            if let Some(term) = term {
                let direction =
                    Self::get_first_child_matching(arena, act, BulletMLNode::match_direction);
                if let Some((dir_type, dir)) = direction {
                    let term = self.get_number_contents(term, data, runner) as u32;
                    let (dir, seq) = if let Some(DirectionType::Sequence) = dir_type {
                        (self.get_number_contents(dir, data, runner), true)
                    } else {
                        (self.get_direction(dir_type, dir, data, runner), false)
                    };
                    self.calc_change_direction(dir, term, seq, data, runner);
                }
            }
        }
        self.act = None;
    }

    fn calc_change_direction<D>(
        &mut self,
        direction: f64,
        term: u32,
        seq: bool,
        data: &RunnerData<D>,
        runner: &dyn AppRunner<D>,
    ) {
        let act_turn = self.act_turn.unwrap_or(0);
        let final_turn = act_turn + term;
        let dir_first = runner.get_bullet_direction(data.data);
        if seq {
            self.change_dir = Some(LinearFunc::new(
                act_turn,
                final_turn,
                dir_first,
                dir_first + direction * f64::from(term),
            ));
        } else {
            let dir_space1 = direction - dir_first;
            let dir_space2 = if dir_space1 > 0. {
                dir_space1 - 360.
            } else {
                dir_space1 + 360.
            };
            let dir_space = if f64::abs(dir_space1) < f64::abs(dir_space2) {
                dir_space1
            } else {
                dir_space2
            };
            self.change_dir = Some(LinearFunc::new(
                act_turn,
                final_turn,
                dir_first,
                dir_first + dir_space,
            ));
        }
    }

    fn run_change_speed<D>(&mut self, data: &mut RunnerData<D>, runner: &dyn AppRunner<D>) {
        if let Some(act) = self.act {
            let arena = &data.bml.arena;
            let term = Self::get_first_child_matching(arena, act, BulletMLNode::match_term);
            if let Some(term) = term {
                let speed = Self::get_first_child_matching(arena, act, BulletMLNode::match_speed);
                if let Some((spd_type, spd)) = speed {
                    let term = self.get_number_contents(term, data, runner) as u32;
                    let spd = if let Some(SpeedType::Sequence) = spd_type {
                        self.get_number_contents(spd, data, runner) * f64::from(term)
                            + runner.get_bullet_speed(data.data)
                    } else {
                        self.get_speed(spd_type, spd, data, runner)
                    };
                    self.calc_change_speed(spd, term, data, runner);
                }
            }
        }
        self.act = None;
    }

    fn calc_change_speed<D>(
        &mut self,
        speed: f64,
        term: u32,
        data: &RunnerData<D>,
        runner: &dyn AppRunner<D>,
    ) {
        let act_turn = self.act_turn.unwrap_or(0);
        let final_turn = act_turn + term;
        let spd_first = runner.get_bullet_speed(data.data);
        self.change_spd = Some(LinearFunc::new(act_turn, final_turn, spd_first, speed));
    }

    fn run_accel<D>(&mut self, data: &mut RunnerData<D>, runner: &dyn AppRunner<D>) {
        if let Some(act) = self.act {
            let arena = &data.bml.arena;
            let term = Self::get_first_child_matching(arena, act, BulletMLNode::match_term);
            if let Some(term) = term {
                let term = self.get_number_contents(term, data, runner) as u32;
                let horizontal =
                    Self::get_first_child_matching(arena, act, BulletMLNode::match_horizontal);
                let vertical =
                    Self::get_first_child_matching(arena, act, BulletMLNode::match_vertical);
                if self.bml_type == Some(BulletMLType::Horizontal) {
                    if let Some((v_type, v)) = vertical {
                        self.accel_x = self.calc_accel_xy(
                            runner.get_bullet_speed_x(),
                            self.get_number_contents(v, data, runner),
                            term,
                            v_type,
                        );
                    }
                    if let Some((h_type, h)) = horizontal {
                        self.accel_y = self.calc_accel_xy(
                            runner.get_bullet_speed_y(),
                            self.get_number_contents(h, data, runner),
                            term,
                            h_type,
                        );
                    }
                } else {
                    if let Some((h_type, h)) = horizontal {
                        self.accel_x = self.calc_accel_xy(
                            runner.get_bullet_speed_x(),
                            self.get_number_contents(h, data, runner),
                            term,
                            h_type,
                        );
                    }
                    if let Some((v_type, v)) = vertical {
                        self.accel_y = self.calc_accel_xy(
                            runner.get_bullet_speed_y(),
                            self.get_number_contents(v, data, runner),
                            term,
                            v_type,
                        );
                    }
                }
            }
        }
        self.act = None;
    }

    fn calc_accel_xy(
        &self,
        first_spd: f64,
        value: f64,
        term: u32,
        hv_type: HVType,
    ) -> Option<LinearFunc<u32, f64>> {
        let act_turn = self.act_turn.unwrap_or(0);
        let final_turn = act_turn + term;
        let final_spd = match hv_type {
            HVType::Sequence => first_spd + value * f64::from(term),
            HVType::Relative => first_spd + value,
            HVType::Absolute => value,
        };
        Some(LinearFunc::new(act_turn, final_turn, first_spd, final_spd))
    }

    fn run_vanish<D>(&mut self, data: &mut RunnerData<D>, runner: &mut dyn AppRunner<D>) {
        runner.do_vanish(data.data);
        self.act = None;
    }

    fn get_parameters<D>(&self, data: &mut RunnerData<D>, runner: &dyn AppRunner<D>) -> Parameters {
        let children = self.act.unwrap().children(&data.bml.arena);
        let mut parameters = Vec::new();
        for child in children {
            let child_node = &data.bml.arena[child];
            if let BulletMLNode::Param(expr) = child_node.get() {
                parameters.push(self.get_number_contents(*expr, data, runner));
            }
        }
        parameters
    }

    fn get_number_contents<D>(
        &self,
        expr: BulletMLExpression,
        data: &mut RunnerData<D>,
        runner: &dyn AppRunner<D>,
    ) -> f64 {
        match expr {
            BulletMLExpression::Const(value) => value,
            BulletMLExpression::Expr(expr) => {
                let rank = runner.get_rank(data.data);
                let expr_ref = expr.from(&data.bml.expr_slab.ps);
                use fasteval::Evaler;
                expr_ref
                    .eval(
                        &data.bml.expr_slab,
                        &mut |name: &str, args: Vec<f64>| match (name, args.as_slice()) {
                            ("v", &[i]) => Some(self.parameters[i as usize - 1]),
                            ("rank", &[]) => Some(rank),
                            ("rand", &[]) => Some(runner.get_rand(data.data)),
                            _ => None,
                        },
                    )
                    .unwrap()
            }
        }
    }
}

#[derive(Debug)]
struct RepeatElem {
    iter: usize,
    end: usize,
    act: NodeId,
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use crate::parse::BulletMLParser;
    use crate::tree::{BulletML, BulletMLNode};

    use super::*;

    pub struct TestAppRunner {
        index: usize,
        turn: u32,
        new_runners: Vec<Runner<TestAppRunner>>,
    }

    impl From<Runner<TestAppRunner>> for TestAppRunner {
        fn from(runner: Runner<TestAppRunner>) -> Self {
            runner.app_runner
        }
    }

    struct TestLog {
        log: Vec<String>,
        pos: usize,
        var_name: String,
    }

    impl TestLog {
        fn new(var_name: String) -> Self {
            TestLog {
                log: Vec::new(),
                pos: 0,
                var_name,
            }
        }

        fn assert_log(&mut self, value: &str, times: usize) {
            if self.pos + times > self.log.len() {
                panic!("too far {} > {}", self.pos + times, self.log.len());
            }
            for val in &self.log[self.pos..(self.pos + times)] {
                assert_eq!(val, value);
            }
            self.pos += times;
        }

        fn assert_log_end(&mut self) {
            let mut pos = self.pos;
            if pos < self.log.len() {
                println!("{} at position {}", self.var_name, pos);
            }
            while pos < self.log.len() {
                let value = &self.log[pos];
                let mut count = 1;
                for val in &self.log[pos + 1..] {
                    if val == value {
                        count += 1;
                    } else {
                        break;
                    }
                }
                println!(
                    "    {}.assert_log(r#\"{}\"#, {});",
                    self.var_name, value, count
                );
                pos += count;
            }
            assert_eq!(self.pos, self.log.len());
            self.log.truncate(0);
        }
    }

    struct TestLogs(Vec<TestLog>);

    impl Drop for TestLogs {
        fn drop(&mut self) {
            for log in &mut self.0 {
                log.assert_log_end();
            }
        }
    }

    impl TestAppRunner {
        pub fn new(index: usize) -> Self {
            TestAppRunner {
                index,
                turn: 0,
                new_runners: Vec::new(),
            }
        }

        pub fn next_turn(&mut self) {
            self.turn += 1;
        }

        fn log_iteration(&mut self, iteration: u32, logs: &mut Vec<TestLog>) {
            if self.index >= logs.len() {
                logs.push(TestLog::new(format!("logs[{}]", self.index)));
            }
            logs[self.index].log.push(format!("=== {}", iteration));
        }
    }

    struct TestAppData<'a> {
        logs: &'a mut Vec<TestLog>,
    }

    impl<'a> AppRunner<TestAppData<'a>> for TestAppRunner {
        fn get_bullet_direction(&self, _data: &TestAppData<'a>) -> f64 {
            0.
        }

        fn get_aim_direction(&self, _data: &TestAppData<'a>) -> f64 {
            0.
        }

        fn get_bullet_speed(&self, _data: &TestAppData<'a>) -> f64 {
            1.
        }

        fn get_default_speed(&self) -> f64 {
            10.
        }

        fn get_rank(&self, _data: &TestAppData<'a>) -> f64 {
            1.
        }

        fn create_simple_bullet(&mut self, data: &mut TestAppData<'a>, direction: f64, speed: f64) {
            data.logs[self.index]
                .log
                .push(format!("create_simple_bullet({}, {})", direction, speed));
        }

        fn create_bullet(
            &mut self,
            data: &mut TestAppData<'a>,
            state: State,
            direction: f64,
            speed: f64,
        ) {
            data.logs[self.index]
                .log
                .push(format!("create_bullet({}, {})", direction, speed));
            let runner = Runner::new_from_state(TestAppRunner::new(0), state);
            self.new_runners.push(runner);
        }

        fn get_turn(&self, _data: &TestAppData<'a>) -> u32 {
            self.turn
        }

        fn do_vanish(&mut self, _data: &mut TestAppData<'a>) {}

        fn do_change_direction(&mut self, data: &mut TestAppData<'a>, direction: f64) {
            data.logs[self.index]
                .log
                .push(format!("do_change_direction({})", direction));
        }

        fn do_change_speed(&mut self, data: &mut TestAppData<'a>, speed: f64) {
            data.logs[self.index]
                .log
                .push(format!("do_change_speed({})", speed));
        }

        fn get_rand(&self, _data: &mut TestAppData<'a>) -> f64 {
            0.42
        }

        fn log(&mut self, data: &mut TestAppData<'a>, node: &BulletMLNode) {
            data.logs[self.index].log.push(format!("{:?}", node));
        }
    }

    struct TestManager {
        bml: BulletML,
        runners: Vec<Runner<TestAppRunner>>,
    }

    impl<'a> TestManager {
        fn new(bml: BulletML) -> Self {
            TestManager {
                bml,
                runners: Vec::new(),
            }
        }

        fn run(&mut self, iteration: u32, logs: &mut Vec<TestLog>) {
            let mut new_runners = Vec::new();
            for runner in &mut self.runners {
                if !runner.is_end() {
                    runner.app_runner.log_iteration(iteration, logs);
                    runner.run(&mut RunnerData {
                        bml: &self.bml,
                        data: &mut TestAppData { logs },
                    });
                    new_runners.extend(&mut runner.new_runners.drain(..));
                    runner.app_runner.next_turn();
                }
            }
            self.runners.reserve(new_runners.len());
            for mut runner in new_runners.drain(..) {
                runner.app_runner.index = self.runners.len();
                self.runners.push(runner);
            }
        }

        fn run_test(&mut self, max_iter: u32, logs: &mut Vec<TestLog>) {
            let runner = Runner::new(TestAppRunner::new(self.runners.len()), &self.bml);
            self.runners.push(runner);
            for i in 0..max_iter {
                self.run(i, logs);
            }
        }
    }

    #[test]
    fn test_mini() {
        let bml = BulletMLParser::new()
            .parse(
                r##"<?xml version="1.0" ?>
<!DOCTYPE bulletml SYSTEM "../../bulletml.dtd">
<bulletml>
<action label="top">
    <fire>
        <bullet />
    </fire>
</action>
</bulletml>"##,
            )
            .unwrap();
        let mut manager = TestManager::new(bml);
        let mut logs = Vec::new();
        manager.run_test(100, &mut logs);
        logs[0].assert_log(r#"=== 0"#, 1);
        logs[0].assert_log(r#"Action(Some("top"))"#, 1);
        logs[0].assert_log(r#"Fire(None)"#, 1);
        logs[0].assert_log(r#"Bullet(None)"#, 1);
        logs[0].assert_log(r#"create_simple_bullet(0, 10)"#, 1);
        logs[0].assert_log(r#"=== 1"#, 1);
        TestLogs(logs);
    }

    #[test]
    fn test_mini_aim() {
        let bml = BulletMLParser::new()
            .parse(
                r##"<?xml version="1.0" ?>
<!DOCTYPE bulletml SYSTEM "../../bulletml.dtd">
<bulletml>
<action label="top">
    <repeat>
        <times>1000</times>
        <action>
            <fire>
                <bullet>
                    <direction type="aim">0</direction>
                    <speed>1</speed>
                </bullet>
            </fire>
            <wait>100</wait>
        </action>
    </repeat>
</action>
</bulletml>"##,
            )
            .unwrap();
        let mut manager = TestManager::new(bml);
        let mut logs = Vec::new();
        manager.run_test(110000, &mut logs);
        logs[0].assert_log(r#"=== 0"#, 1);
        logs[0].assert_log(r#"Action(Some("top"))"#, 1);
        logs[0].assert_log(r#"Repeat"#, 1);
        for i in 0..1000 {
            logs[0].assert_log(r#"Action(None)"#, 1);
            logs[0].assert_log(r#"Fire(None)"#, 1);
            logs[0].assert_log(r#"Bullet(None)"#, 1);
            logs[0].assert_log(r#"create_simple_bullet(0, 1)"#, 1);
            logs[0].assert_log(r#"Wait(Const(100.0))"#, 1);
            for j in 0..100 {
                logs[0].assert_log(&format!(r#"=== {}"#, i * 100 + j + 1), 1);
            }
        }
        TestLogs(logs);
    }

    #[test]
    fn test_bulletsmorph_double_seduction() {
        let bml = BulletMLParser::with_capacities(12, 128)
            .parse(
                r##"<?xml version="1.0" ?>
    <!DOCTYPE bulletml SYSTEM "../bulletml.dtd">
    <bulletml type="vertical" xmlns="http://www.asahi-net.or.jp/~cs8k-cyu/bulletml">
    <action label="top">
        <fire>
            <direction type="aim">30</direction>
            <bulletRef label="parentbit">
                <param>1</param>
            </bulletRef>
        </fire>
        <fire>
            <direction type="aim">-30</direction>
            <bulletRef label="parentbit">
                <param>-1</param>
            </bulletRef>
        </fire>
        <wait>300</wait>
    </action>
    <bullet label="parentbit">
        <speed>2.0</speed>
        <action>
            <actionRef label="cross">
                <param>75</param>
                <param>0</param>
            </actionRef>
            <actionRef label="cross">
                <param>70</param>
                <param>0</param>
            </actionRef>
            <actionRef label="cross">
                <param>65</param>
                <param>0</param>
            </actionRef>
            <actionRef label="cross">
                <param>60</param>
                <param>0</param>
            </actionRef>
            <actionRef label="cross">
               <param>55</param>
                <param>0</param>
            </actionRef>
            <actionRef label="cross">
                <param>50</param>
                <param>0</param>
            </actionRef>
            <actionRef label="cross">
                <param>80</param>
                <param>15 * $1</param>
            </actionRef>
                <actionRef label="cross">
                <param>75</param>
                <param>10 * $1</param>
            </actionRef>
            <actionRef label="cross">
                <param>70</param>
                <param>6 * $1</param>
            </actionRef>
            <actionRef label="cross">
                <param>65</param>
                <param>3 * $1</param>
            </actionRef>
            <actionRef label="cross">
                <param>60</param>
                <param>1 * $1</param>
            </actionRef>
            <actionRef label="cross">
                <param>55</param>
                <param>0</param>
            </actionRef>
            <vanish/>
        </action>
    </bullet>
    <action label="cross">
        <fire>
            <direction type="absolute">0</direction>
            <bulletRef label="aimbit">
                <param>$1</param>
                <param>$2</param>
            </bulletRef>
        </fire>
        <fire>
            <direction type="absolute">90</direction>
            <bulletRef label="aimbit">
                <param>$1</param>
                <param>$2</param>
            </bulletRef>
        </fire>
        <fire>
            <direction type="absolute">180</direction>
            <bulletRef label="aimbit">
                <param>$1</param>
                <param>$2</param>
            </bulletRef>
        </fire>
        <fire>
            <direction type="absolute">270</direction>
            <bulletRef label="aimbit">
                <param>$1</param>
                <param>$2</param>
            </bulletRef>
        </fire>
        <wait>5</wait>
    </action>
    <bullet label="aimbit">
        <speed>0.6</speed>
        <action>
            <wait>$1</wait>
            <fire>
                <direction type="aim">$2</direction>
                <speed>1.6 * (0.5 + 0.5 * $rank)</speed>
                <bullet/>
            </fire>
            <repeat>
                <times>2 + 5 * $rank</times>
                <action>
                    <fire>
                        <direction type="sequence">0</direction>
                        <speed type="sequence">0.1</speed>
                        <bullet/>
                    </fire>
                </action>
            </repeat>
            <vanish/>
        </action>
    </bullet>
    </bulletml>"##,
            )
            .unwrap();
        let mut manager = TestManager::new(bml);
        let mut logs = Vec::new();
        manager.run_test(1000, &mut logs);
        logs[0].assert_log(r#"=== 0"#, 1);
        logs[0].assert_log(r#"Action(Some("top"))"#, 1);
        logs[0].assert_log(r#"Fire(None)"#, 1);
        logs[0].assert_log(r#"BulletRef("parentbit")"#, 1);
        logs[0].assert_log(r#"Bullet(Some("parentbit"))"#, 1);
        logs[0].assert_log(r#"create_bullet(30, 2)"#, 1);
        logs[0].assert_log(r#"Fire(None)"#, 1);
        logs[0].assert_log(r#"BulletRef("parentbit")"#, 1);
        logs[0].assert_log(r#"Bullet(Some("parentbit"))"#, 1);
        logs[0].assert_log(r#"create_bullet(330, 2)"#, 1);
        logs[0].assert_log(r#"Wait(Const(300.0))"#, 1);
        for i in 0..300 {
            logs[0].assert_log(&format!(r#"=== {}"#, i + 1), 1);
        }

        #[allow(clippy::needless_range_loop)]
        for i in 1..3 {
            logs[i].assert_log(r#"=== 1"#, 1);
            logs[i].assert_log(r#"Action(None)"#, 1);
            for j in 0..12 {
                logs[i].assert_log(r#"ActionRef("cross")"#, 1);
                logs[i].assert_log(r#"Action(Some("cross"))"#, 1);
                for k in 0..4 {
                    logs[i].assert_log(r#"Fire(None)"#, 1);
                    logs[i].assert_log(r#"BulletRef("aimbit")"#, 1);
                    logs[i].assert_log(r#"Bullet(Some("aimbit"))"#, 1);
                    logs[i].assert_log(&format!(r#"create_bullet({}, 0.6)"#, k * 90 % 360), 1);
                }
                logs[i].assert_log(r#"Wait(Const(5.0))"#, 1);
                for k in 0..5 {
                    logs[i].assert_log(&format!(r#"=== {}"#, j * 5 + k + 2), 1);
                }
            }
            logs[i].assert_log(r#"Vanish"#, 1);
            logs[i].assert_log(&format!(r#"=== {}"#, 62), 1);
        }

        let v1s = [75, 70, 65, 60, 55, 50, 80, 75, 70, 65, 60, 55];
        let v2_factors = [0, 0, 0, 0, 0, 0, 15, 10, 6, 3, 1, 0];
        for i in 3..99 {
            logs[i].assert_log(&format!(r#"=== {}"#, (i - 3) / 8 * 5 + 2), 1);
            let mut spd = 1.6;
            for j in 0..1 {
                logs[i].assert_log(r#"Action(None)"#, 1);
                logs[i].assert_log(r#"Wait(Expr(ExpressionI(27)))"#, 1);
                for k in 0..v1s[(i - 3) / 8 % 12] {
                    logs[i].assert_log(&format!(r#"=== {}"#, (i - 3) / 8 * 5 + k + 3), 1);
                }
                logs[i].assert_log(r#"Fire(None)"#, 1);
                logs[i].assert_log(r#"Bullet(None)"#, 1);
                let mut dir = v2_factors[j + (i - 3) / 8] * (((i - 3) % 8 / 4) as isize * -2 + 1);
                if dir > 360 {
                    dir -= 360;
                }
                if dir < 0 {
                    dir += 360;
                }
                logs[i].assert_log(&format!(r#"create_simple_bullet({}, {})"#, dir, spd), 1);
                logs[i].assert_log(r#"Repeat"#, 1);
                for _ in 0..7 {
                    logs[i].assert_log(r#"Action(None)"#, 1);
                    logs[i].assert_log(r#"Fire(None)"#, 1);
                    logs[i].assert_log(r#"Bullet(None)"#, 1);
                    spd += 0.1;
                    logs[i].assert_log(&format!(r#"create_simple_bullet({}, {})"#, dir, spd), 1);
                }
                logs[i].assert_log(r#"Vanish"#, 1);
            }
            logs[i].assert_log(
                &format!(r#"=== {}"#, (i - 3) / 8 * 5 + v1s[(i - 3) / 8 % 12] + 3),
                1,
            );
        }
        TestLogs(logs);
    }

    #[test]
    fn test_tt_morph_0to1() {
        let bml = BulletMLParser::new()
            .parse(
                r##"<?xml version="1.0" ?>
<!DOCTYPE bulletml SYSTEM "http://www.asahi-net.or.jp/~cs8k-cyu/bulletml/bulletml.dtd">

<bulletml type="vertical"
          xmlns="http://www.asahi-net.or.jp/~cs8k-cyu/bulletml">

<action label="top">
        <changeSpeed>
                <speed>0</speed>
                <term>1</term>
        </changeSpeed>
        <wait>1</wait>
        <changeSpeed>
                <speed>1</speed>
                <term>60-$rank*50</term>
        </changeSpeed>
        <wait>60-$rank*50</wait>
        <fire>
                <direction type="relative">0</direction>
                <bullet/>
        </fire>
</action>

</bulletml>"##,
            )
            .unwrap();
        let mut manager = TestManager::new(bml);
        let mut logs = Vec::new();
        manager.run_test(100, &mut logs);
        logs[0].assert_log(r#"=== 0"#, 1);
        logs[0].assert_log(r#"Action(Some("top"))"#, 1);
        logs[0].assert_log(r#"ChangeSpeed"#, 1);
        logs[0].assert_log(r#"Wait(Const(1.0))"#, 1);
        logs[0].assert_log(r#"=== 1"#, 1);
        logs[0].assert_log(r#"do_change_speed(0)"#, 1);
        logs[0].assert_log(r#"ChangeSpeed"#, 1);
        logs[0].assert_log(r#"Wait(Expr(ExpressionI(1)))"#, 1);
        logs[0].assert_log(r#"=== 2"#, 1);
        logs[0].assert_log(r#"do_change_speed(1)"#, 1);
        logs[0].assert_log(r#"=== 3"#, 1);
        logs[0].assert_log(r#"do_change_speed(1)"#, 1);
        logs[0].assert_log(r#"=== 4"#, 1);
        logs[0].assert_log(r#"do_change_speed(1)"#, 1);
        logs[0].assert_log(r#"=== 5"#, 1);
        logs[0].assert_log(r#"do_change_speed(1)"#, 1);
        logs[0].assert_log(r#"=== 6"#, 1);
        logs[0].assert_log(r#"do_change_speed(1)"#, 1);
        logs[0].assert_log(r#"=== 7"#, 1);
        logs[0].assert_log(r#"do_change_speed(1)"#, 1);
        logs[0].assert_log(r#"=== 8"#, 1);
        logs[0].assert_log(r#"do_change_speed(1)"#, 1);
        logs[0].assert_log(r#"=== 9"#, 1);
        logs[0].assert_log(r#"do_change_speed(1)"#, 1);
        logs[0].assert_log(r#"=== 10"#, 1);
        logs[0].assert_log(r#"do_change_speed(1)"#, 1);
        logs[0].assert_log(r#"=== 11"#, 1);
        logs[0].assert_log(r#"do_change_speed(1)"#, 1);
        logs[0].assert_log(r#"Fire(None)"#, 1);
        logs[0].assert_log(r#"Bullet(None)"#, 1);
        logs[0].assert_log(r#"create_simple_bullet(0, 10)"#, 1);
        logs[0].assert_log(r#"=== 12"#, 1);
        TestLogs(logs);
    }

    #[test]
    fn test_tt_morph_accelshot() {
        let bml = BulletMLParser::new()
            .parse(
                r##"<?xml version="1.0" ?>
<!DOCTYPE bulletml SYSTEM "http://www.asahi-net.or.jp/~cs8k-cyu/bulletml/bulletml.dtd">

<bulletml type="vertical"
          xmlns="http://www.asahi-net.or.jp/~cs8k-cyu/bulletml">

<action label="top">
        <fire>
                <direction type="relative">0</direction>
                <speed type="relative">-0.9</speed>
                <bulletRef label="accel"/>
        </fire>
        <repeat><times>$rank*1.7</times>
        <action>
                <wait>2</wait>
                <fire>
                        <direction type="relative">0</direction>
                        <speed type="sequence">0.3</speed>
                        <bulletRef label="accel"/>
                </fire>
        </action>
        </repeat>
        <vanish/>
</action>

<bullet label="accel">
        <action>
                <wait>3</wait>
                <changeSpeed>
                        <speed>1</speed>
                        <term>60</term>
                </changeSpeed>
        </action>
</bullet>

</bulletml>"##,
            )
            .unwrap();
        let mut manager = TestManager::new(bml);
        let mut logs = Vec::new();
        manager.run_test(100, &mut logs);
        logs[0].assert_log(r#"=== 0"#, 1);
        logs[0].assert_log(r#"Action(Some("top"))"#, 1);
        logs[0].assert_log(r#"Fire(None)"#, 1);
        logs[0].assert_log(r#"BulletRef("accel")"#, 1);
        logs[0].assert_log(r#"Bullet(Some("accel"))"#, 1);
        logs[0].assert_log(r#"create_bullet(0, 0.09999999999999998)"#, 1);
        logs[0].assert_log(r#"Repeat"#, 1);
        logs[0].assert_log(r#"Action(None)"#, 1);
        logs[0].assert_log(r#"Wait(Const(2.0))"#, 1);
        logs[0].assert_log(r#"=== 1"#, 1);
        logs[0].assert_log(r#"=== 2"#, 1);
        logs[0].assert_log(r#"Fire(None)"#, 1);
        logs[0].assert_log(r#"BulletRef("accel")"#, 1);
        logs[0].assert_log(r#"Bullet(Some("accel"))"#, 1);
        logs[0].assert_log(r#"create_bullet(0, 0.39999999999999997)"#, 1);
        logs[0].assert_log(r#"Vanish"#, 1);
        logs[0].assert_log(r#"=== 3"#, 1);

        logs[1].assert_log(r#"=== 1"#, 1);
        logs[1].assert_log(r#"Action(None)"#, 1);
        logs[1].assert_log(r#"Wait(Const(3.0))"#, 1);
        logs[1].assert_log(r#"=== 2"#, 1);
        logs[1].assert_log(r#"=== 3"#, 1);
        logs[1].assert_log(r#"=== 4"#, 1);
        logs[1].assert_log(r#"ChangeSpeed"#, 1);
        for i in 0..60 {
            logs[1].assert_log(&format!(r#"=== {}"#, i + 5), 1);
            logs[1].assert_log(r#"do_change_speed(1)"#, 1);
        }

        logs[2].assert_log(r#"=== 3"#, 1);
        logs[2].assert_log(r#"Action(None)"#, 1);
        logs[2].assert_log(r#"Wait(Const(3.0))"#, 1);
        logs[2].assert_log(r#"=== 4"#, 1);
        logs[2].assert_log(r#"=== 5"#, 1);
        logs[2].assert_log(r#"=== 6"#, 1);
        logs[2].assert_log(r#"ChangeSpeed"#, 1);
        for i in 0..60 {
            logs[2].assert_log(&format!(r#"=== {}"#, i + 7), 1);
            logs[2].assert_log(r#"do_change_speed(1)"#, 1);
        }
        TestLogs(logs);
    }

    #[test]
    fn test_tt_morph_twin() {
        let bml = BulletMLParser::new()
            .parse(
                r##"<?xml version="1.0" ?>
<!DOCTYPE bulletml SYSTEM "http://www.asahi-net.or.jp/~cs8k-cyu/bulletml/bulletml.dtd">

<bulletml type="vertical"
          xmlns="http://www.asahi-net.or.jp/~cs8k-cyu/bulletml">


 <action label="top">
  <wait>1</wait>
  <fire>
   <bullet>
        <direction type="relative">0</direction>
        <speed type="relative">$rank</speed>
    <actionRef label="ofs">
     <param>90</param>
    </actionRef>
   </bullet>
  </fire>
  <fire>
   <bullet>
        <direction type="relative">0</direction>
        <speed type="relative">$rank</speed>
    <actionRef label="ofs">
     <param>-90</param>
    </actionRef>
   </bullet>
  </fire>
  <vanish/>
 </action>

<action label="ofs">
  <changeDirection>
   <direction type="relative">$1</direction>
   <term>1</term>
  </changeDirection>
  <wait>1</wait>
  <changeDirection>
   <direction type="relative">0-$1</direction>
   <term>1</term>
  </changeDirection>
  <wait>1</wait>
  <fire>
        <direction type="relative">0</direction>
        <speed type="relative">-$rank</speed>
        <bullet/>
  </fire>
  <vanish/>
</action>

</bulletml>"##,
            )
            .unwrap();
        let mut manager = TestManager::new(bml);
        let mut logs = Vec::new();
        manager.run_test(100, &mut logs);
        logs[0].assert_log(r#"=== 0"#, 1);
        logs[0].assert_log(r#"Action(Some("top"))"#, 1);
        logs[0].assert_log(r#"Wait(Const(1.0))"#, 1);
        logs[0].assert_log(r#"=== 1"#, 1);
        logs[0].assert_log(r#"Fire(None)"#, 1);
        logs[0].assert_log(r#"Bullet(None)"#, 1);
        logs[0].assert_log(r#"create_bullet(0, 2)"#, 1);
        logs[0].assert_log(r#"Fire(None)"#, 1);
        logs[0].assert_log(r#"Bullet(None)"#, 1);
        logs[0].assert_log(r#"create_bullet(0, 2)"#, 1);
        logs[0].assert_log(r#"Vanish"#, 1);
        logs[0].assert_log(r#"=== 2"#, 1);

        logs[1].assert_log(r#"=== 2"#, 1);
        logs[1].assert_log(r#"ActionRef("ofs")"#, 1);
        logs[1].assert_log(r#"Action(Some("ofs"))"#, 1);
        logs[1].assert_log(r#"ChangeDirection"#, 1);
        logs[1].assert_log(r#"Wait(Const(1.0))"#, 1);
        logs[1].assert_log(r#"=== 3"#, 1);
        logs[1].assert_log(r#"do_change_direction(90)"#, 1);
        logs[1].assert_log(r#"ChangeDirection"#, 1);
        logs[1].assert_log(r#"Wait(Const(1.0))"#, 1);
        logs[1].assert_log(r#"=== 4"#, 1);
        logs[1].assert_log(r#"do_change_direction(-90)"#, 1);
        logs[1].assert_log(r#"Fire(None)"#, 1);
        logs[1].assert_log(r#"Bullet(None)"#, 1);
        logs[1].assert_log(r#"create_simple_bullet(0, 0)"#, 1);
        logs[1].assert_log(r#"Vanish"#, 1);
        logs[1].assert_log(r#"=== 5"#, 1);

        logs[2].assert_log(r#"=== 2"#, 1);
        logs[2].assert_log(r#"ActionRef("ofs")"#, 1);
        logs[2].assert_log(r#"Action(Some("ofs"))"#, 1);
        logs[2].assert_log(r#"ChangeDirection"#, 1);
        logs[2].assert_log(r#"Wait(Const(1.0))"#, 1);
        logs[2].assert_log(r#"=== 3"#, 1);
        logs[2].assert_log(r#"do_change_direction(-90)"#, 1);
        logs[2].assert_log(r#"ChangeDirection"#, 1);
        logs[2].assert_log(r#"Wait(Const(1.0))"#, 1);
        logs[2].assert_log(r#"=== 4"#, 1);
        logs[2].assert_log(r#"do_change_direction(90)"#, 1);
        logs[2].assert_log(r#"Fire(None)"#, 1);
        logs[2].assert_log(r#"Bullet(None)"#, 1);
        logs[2].assert_log(r#"create_simple_bullet(0, 0)"#, 1);
        logs[2].assert_log(r#"Vanish"#, 1);
        logs[2].assert_log(r#"=== 5"#, 1);
        TestLogs(logs);
    }

    #[test]
    fn test_tt_morph_wedge_half() {
        let bml = BulletMLParser::new()
            .parse(
                r##"<?xml version="1.0" ?>
<!DOCTYPE bulletml SYSTEM "http://www.asahi-net.or.jp/~cs8k-cyu/bulletml/bulletml.dtd">

<bulletml type="vertical"
          xmlns="http://www.asahi-net.or.jp/~cs8k-cyu/bulletml">

 <action label="top">
  <wait>1</wait>
  <fire>
   <bullet>
        <direction type="relative">0</direction>
        <speed type="relative">$rank*0.4+0.2</speed>
    <actionRef label="ofs">
     <param>0</param>
     <param>-0.08</param>
    </actionRef>
   </bullet>
  </fire>
  <fire>
   <bullet>
        <direction type="relative">0</direction>
        <speed type="relative">$rank*0.4+0.2</speed>
    <actionRef label="ofs">
     <param>-120</param>
     <param>0.08</param>
    </actionRef>
   </bullet>
  </fire>
  <vanish/>
 </action>

<action label="ofs">
  <changeDirection>
   <direction type="relative">$1</direction>
   <term>1</term>
  </changeDirection>
  <wait>1</wait>
  <changeDirection>
   <direction type="relative">0-$1</direction>
   <term>1</term>
  </changeDirection>
  <wait>1</wait>
  <fire>
        <direction type="relative">0</direction>
        <speed type="relative">$2-$rank*0.4-0.2</speed>
        <bullet/>
  </fire>
  <vanish/>
</action>

</bulletml>"##,
            )
            .unwrap();
        let mut manager = TestManager::new(bml);
        let mut logs = Vec::new();
        manager.run_test(100, &mut logs);
        logs[0].assert_log(r#"=== 0"#, 1);
        logs[0].assert_log(r#"Action(Some("top"))"#, 1);
        logs[0].assert_log(r#"Wait(Const(1.0))"#, 1);
        logs[0].assert_log(r#"=== 1"#, 1);
        logs[0].assert_log(r#"Fire(None)"#, 1);
        logs[0].assert_log(r#"Bullet(None)"#, 1);
        logs[0].assert_log(r#"create_bullet(0, 1.6)"#, 1);
        logs[0].assert_log(r#"Fire(None)"#, 1);
        logs[0].assert_log(r#"Bullet(None)"#, 1);
        logs[0].assert_log(r#"create_bullet(0, 1.6)"#, 1);
        logs[0].assert_log(r#"Vanish"#, 1);
        logs[0].assert_log(r#"=== 2"#, 1);

        logs[1].assert_log(r#"=== 2"#, 1);
        logs[1].assert_log(r#"ActionRef("ofs")"#, 1);
        logs[1].assert_log(r#"Action(Some("ofs"))"#, 1);
        logs[1].assert_log(r#"ChangeDirection"#, 1);
        logs[1].assert_log(r#"Wait(Const(1.0))"#, 1);
        logs[1].assert_log(r#"=== 3"#, 1);
        logs[1].assert_log(r#"do_change_direction(0)"#, 1);
        logs[1].assert_log(r#"ChangeDirection"#, 1);
        logs[1].assert_log(r#"Wait(Const(1.0))"#, 1);
        logs[1].assert_log(r#"=== 4"#, 1);
        logs[1].assert_log(r#"do_change_direction(0)"#, 1);
        logs[1].assert_log(r#"Fire(None)"#, 1);
        logs[1].assert_log(r#"Bullet(None)"#, 1);
        logs[1].assert_log(r#"create_simple_bullet(0, 0.31999999999999995)"#, 1);
        logs[1].assert_log(r#"Vanish"#, 1);
        logs[1].assert_log(r#"=== 5"#, 1);

        logs[2].assert_log(r#"=== 2"#, 1);
        logs[2].assert_log(r#"ActionRef("ofs")"#, 1);
        logs[2].assert_log(r#"Action(Some("ofs"))"#, 1);
        logs[2].assert_log(r#"ChangeDirection"#, 1);
        logs[2].assert_log(r#"Wait(Const(1.0))"#, 1);
        logs[2].assert_log(r#"=== 3"#, 1);
        logs[2].assert_log(r#"do_change_direction(-120)"#, 1);
        logs[2].assert_log(r#"ChangeDirection"#, 1);
        logs[2].assert_log(r#"Wait(Const(1.0))"#, 1);
        logs[2].assert_log(r#"=== 4"#, 1);
        logs[2].assert_log(r#"do_change_direction(120)"#, 1);
        logs[2].assert_log(r#"Fire(None)"#, 1);
        logs[2].assert_log(r#"Bullet(None)"#, 1);
        logs[2].assert_log(r#"create_simple_bullet(0, 0.48)"#, 1);
        logs[2].assert_log(r#"Vanish"#, 1);
        logs[2].assert_log(r#"=== 5"#, 1);
        TestLogs(logs);
    }
}
