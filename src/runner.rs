use indextree::{Node, NodeId};
use std::collections::HashSet;
use std::ops::{Deref, DerefMut};

use crate::tree::{BulletML, BulletMLNode, BulletMLType, DirectionType, HVType, SpeedType};

pub struct RunnerData<'a, D: 'a> {
    pub bml: &'a BulletML,
    pub data: &'a mut D,
}

type Parameters = Vec<f64>;

pub struct State {
    bml_type: Option<BulletMLType>,
    nodes: Box<[NodeId]>,
    parameters: Parameters,
}

pub struct Runner<R> {
    runners: Vec<RunnerImpl>,
    app_runner: R,
}

impl<'a, R> Runner<R> {
    pub fn new(app_runner: R, bml: &BulletML) -> Self {
        let bml_type = Self::get_bml_type(bml);
        let runners = bml
            .root
            .children(&bml.arena)
            .filter(|child| {
                let child_node = &bml.arena[*child];
                child_node.data.is_top_action()
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

    pub fn init<D>(&mut self, bml: &BulletML)
    where
        R: AppRunner<D>,
    {
        let bml_type = Self::get_bml_type(bml);
        self.runners.clear();
        for action in bml.root.children(&bml.arena).filter(|child| {
            let child_node = &bml.arena[*child];
            child_node.data.is_top_action()
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

    pub fn new_from_state(app_runner: R, state: State) -> Self {
        Runner {
            runners: vec![RunnerImpl::new(state)],
            app_runner,
        }
    }

    pub fn init_from_state<D>(&mut self, state: State)
    where
        R: AppRunner<D>,
    {
        self.runners.clear();
        self.runners.push(RunnerImpl::new(state));
        self.app_runner.init();
    }

    pub fn get_bml_type(bml: &BulletML) -> Option<BulletMLType> {
        let root_node = &bml.arena[bml.root];
        if let BulletMLNode::BulletML { bml_type } = root_node.data {
            bml_type
        } else {
            None
        }
    }

    pub fn run<D>(&mut self, data: &mut RunnerData<D>)
    where
        R: AppRunner<D>,
    {
        for runner in &mut self.runners {
            runner.run(data, &mut self.app_runner);
        }
    }

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

pub trait AppRunner<D> {
    fn init(&mut self) {}
    fn get_bullet_direction(&self, data: &D) -> f64;
    fn get_aim_direction(&self, data: &D) -> f64;
    fn get_bullet_speed(&self, data: &D) -> f64;
    fn get_default_speed(&self) -> f64;
    fn get_rank(&self, data: &D) -> f64;
    fn create_simple_bullet(&mut self, data: &mut D, direction: f64, speed: f64);
    fn create_bullet(&mut self, data: &mut D, state: State, direction: f64, speed: f64);
    fn get_turn(&self, data: &D) -> u32;
    fn do_vanish(&self, data: &mut D);
    fn do_change_direction(&self, _data: &mut D, _direction: f64) {}
    fn do_change_speed(&self, _data: &mut D, _speed: f64) {}
    fn do_accel_x(&self, _: f64) {}
    fn do_accel_y(&self, _: f64) {}
    fn get_bullet_speed_x(&self) -> f64 {
        0.
    }
    fn get_bullet_speed_y(&self) -> f64 {
        0.
    }
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
    ref_stack: Vec<(NodeId, Parameters)>,
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

    fn changes<D>(&mut self, data: &mut RunnerData<D>, runner: &dyn AppRunner<D>) {
        let now = runner.get_turn(data.data);
        let reset = if let Some(change_dir) = &self.change_dir {
            if change_dir.is_last(now) {
                runner.do_change_direction(&mut data.data, change_dir.get_last());
                true
            } else {
                runner.do_change_direction(&mut data.data, change_dir.get_value(now));
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
                runner.do_change_speed(&mut data.data, change_spd.get_last());
                true
            } else {
                runner.do_change_speed(&mut data.data, change_spd.get_value(now));
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
            runner.log(&mut data.data, &node.data);
            match &node.data {
                BulletMLNode::Bullet { .. } => self.run_bullet(data, runner),
                BulletMLNode::Action { .. } => self.run_action(node),
                BulletMLNode::Fire { .. } => self.run_fire(data, runner),
                BulletMLNode::ChangeDirection => self.run_change_direction(data, runner),
                BulletMLNode::ChangeSpeed => self.run_change_speed(data, runner),
                BulletMLNode::Accel => self.run_accel(data, runner),
                BulletMLNode::Wait(expr) => self.run_wait(*expr, data, runner),
                BulletMLNode::Repeat => self.run_repeat(act, data, runner),
                BulletMLNode::BulletRef(label) => {
                    self.run_ref(bml.bullet_refs[label], data, runner)
                }
                BulletMLNode::ActionRef(label) => {
                    self.run_ref(bml.action_refs[label], data, runner)
                }
                BulletMLNode::FireRef(label) => self.run_ref(bml.fire_refs[label], data, runner),
                BulletMLNode::Vanish => self.run_vanish(data, runner),
                _ => (),
            }
            if self.act.is_none() && !self.root_nodes.contains(&prev) {
                let parent = prev_node.parent();
                if let Some(parent) = parent {
                    if let BulletMLNode::BulletML { .. } = bml.arena[parent].data {
                        let top = self.ref_stack.pop().unwrap();
                        prev = top.0;
                        prev_node = &bml.arena[prev];
                        self.parameters = top.1;
                    }
                }
            }
            if self.act.is_none() && !self.root_nodes.contains(&prev) {
                self.act = prev_node.next_sibling();
            }
            while self.act.is_none() {
                if !self.root_nodes.contains(&prev) {
                    let parent = prev_node.parent();
                    if let Some(parent) = parent {
                        if let BulletMLNode::Repeat = bml.arena[parent].data {
                            {
                                let rep = self.repeat_stack.last_mut().unwrap();
                                rep.iter += 1;
                                if rep.iter < rep.end {
                                    self.act = Some(rep.act);
                                    break;
                                }
                            };
                            self.repeat_stack.pop();
                        }
                    }
                    self.act = parent;
                } else {
                    self.act = None;
                }
                match self.act {
                    None => break,
                    Some(act) => {
                        prev = act;
                        prev_node = &bml.arena[prev]
                    }
                }
                if !self.root_nodes.contains(&prev) {
                    let parent = prev_node.parent();
                    if let Some(parent) = parent {
                        if let BulletMLNode::BulletML { .. } = bml.arena[parent].data {
                            let top = self.ref_stack.pop().unwrap();
                            self.act = Some(top.0);
                            prev = top.0;
                            prev_node = &bml.arena[prev];
                            self.parameters = top.1;
                        }
                    }
                }
                self.act = {
                    let act_node = &bml.arena[self.act.unwrap()];
                    if !act_node.data.is_top_action() {
                        act_node.next_sibling()
                    } else {
                        None
                    }
                };
            }
        }
    }

    fn get_first_child_matching<M>(bml: &BulletML, parent: Option<NodeId>, m: M) -> Option<NodeId>
    where
        M: Fn(&BulletMLNode) -> bool,
    {
        if let Some(parent) = parent {
            for child in parent.children(&bml.arena) {
                let child_node = &bml.arena[child];
                if m(&child_node.data) {
                    return Some(child);
                }
            }
        }
        None
    }

    fn get_children_matching<M>(bml: &BulletML, parent: Option<NodeId>, m: M) -> Vec<NodeId>
    where
        M: Fn(&BulletMLNode) -> bool,
    {
        if let Some(parent) = parent {
            parent
                .children(&bml.arena)
                .filter(|child| {
                    let child_node = &bml.arena[*child];
                    m(&child_node.data)
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    fn shot_init(&mut self) {
        self.spd.invalidate();
        self.dir.invalidate();
    }

    fn get_direction<D>(
        &mut self,
        dir_type: Option<DirectionType>,
        expr: fasteval::ExpressionI,
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
            for child in act.children(&data.bml.arena) {
                let child_node = &data.bml.arena[child];
                if let BulletMLNode::Direction { dir_type, dir } = &child_node.data {
                    let direction = self.get_direction(*dir_type, *dir, data, runner);
                    self.dir.set(direction);
                    break;
                }
            }
        }
    }

    fn get_speed<D>(
        &mut self,
        spd_type: Option<SpeedType>,
        expr: fasteval::ExpressionI,
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
            for child in act.children(&data.bml.arena) {
                let child_node = &data.bml.arena[child];
                if let BulletMLNode::Speed { spd_type, spd } = &child_node.data {
                    let speed = self.get_speed(*spd_type, *spd, data, runner);
                    self.spd.set(speed);
                    break;
                }
            }
        }
    }

    fn run_bullet<D>(&mut self, data: &mut RunnerData<D>, runner: &mut dyn AppRunner<D>) {
        let bml = data.bml;
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
        let all_actions = RunnerImpl::get_children_matching(bml, self.act, |node| {
            node.is_action() || node.is_action_ref()
        });
        if all_actions.is_empty() {
            runner.create_simple_bullet(&mut data.data, self.dir.get(), self.spd.get());
        } else {
            let state = State {
                bml_type: self.bml_type,
                nodes: all_actions.into_boxed_slice(),
                parameters: self.parameters.clone(),
            };
            runner.create_bullet(&mut data.data, state, self.dir.get(), self.spd.get());
        }
        self.act = None;
    }

    fn run_fire<D>(&mut self, data: &mut RunnerData<D>, runner: &dyn AppRunner<D>) {
        self.shot_init();
        self.set_speed(data, runner);
        self.set_direction(data, runner);
        let bullet =
            RunnerImpl::get_first_child_matching(data.bml, self.act, BulletMLNode::is_bullet)
                .or_else(|| {
                    RunnerImpl::get_first_child_matching(
                        data.bml,
                        self.act,
                        BulletMLNode::is_bullet_ref,
                    )
                });
        self.act = bullet;
    }

    fn run_action(&mut self, node: &Node<BulletMLNode>) {
        self.act = node.first_child();
    }

    fn run_wait<D>(
        &mut self,
        expr: fasteval::ExpressionI,
        data: &mut RunnerData<D>,
        runner: &dyn AppRunner<D>,
    ) {
        let frame = self.get_number_contents(expr, data, runner);
        self.do_wait(frame as u32);
        self.act = None;
    }

    fn run_repeat<D>(&mut self, act: NodeId, data: &mut RunnerData<D>, runner: &dyn AppRunner<D>) {
        let mut times: Option<usize> = None;
        for child in act.children(&data.bml.arena) {
            let child_node = &data.bml.arena[child];
            if let BulletMLNode::Times(expr) = &child_node.data {
                times = Some(self.get_number_contents(*expr, data, runner) as usize);
                break;
            }
        }
        if let Some(times) = times {
            let action =
                RunnerImpl::get_first_child_matching(data.bml, self.act, BulletMLNode::is_action)
                    .or_else(|| {
                        RunnerImpl::get_first_child_matching(
                            data.bml,
                            self.act,
                            BulletMLNode::is_action_ref,
                        )
                    });
            self.repeat_stack.push(RepeatElem {
                iter: 0,
                end: times,
                act: action.unwrap(),
            });
            self.act = action;
        }
    }

    fn run_ref<D>(&mut self, r: NodeId, data: &mut RunnerData<D>, runner: &dyn AppRunner<D>) {
        let new_parameters = self.get_parameters(data, runner);
        let prev_parameters = std::mem::replace(&mut self.parameters, new_parameters);
        self.ref_stack.push((self.act.unwrap(), prev_parameters));
        self.act = Some(r);
    }

    fn run_change_direction<D>(&mut self, data: &mut RunnerData<D>, runner: &dyn AppRunner<D>) {
        let term_node =
            RunnerImpl::get_first_child_matching(data.bml, self.act, BulletMLNode::is_term)
                .map(|term| &data.bml.arena[term]);
        if let Some(Node {
            data: BulletMLNode::Term(term),
            ..
        }) = &term_node
        {
            let direction_node = RunnerImpl::get_first_child_matching(
                data.bml,
                self.act,
                BulletMLNode::is_direction,
            )
            .map(|direction| &data.bml.arena[direction]);
            if let Some(Node {
                data: BulletMLNode::Direction { dir_type, dir },
                ..
            }) = &direction_node
            {
                let term = self.get_number_contents(*term, data, runner) as u32;
                let (dir, seq) = if let Some(DirectionType::Sequence) = dir_type {
                    (self.get_number_contents(*dir, data, runner), true)
                } else {
                    (self.get_direction(*dir_type, *dir, data, runner), false)
                };
                self.calc_change_direction(dir, term, seq, data, runner);
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
        let term_node =
            RunnerImpl::get_first_child_matching(data.bml, self.act, BulletMLNode::is_term)
                .map(|term| &data.bml.arena[term]);
        if let Some(Node {
            data: BulletMLNode::Term(term),
            ..
        }) = &term_node
        {
            let speed_node =
                RunnerImpl::get_first_child_matching(data.bml, self.act, BulletMLNode::is_speed)
                    .map(|speed| &data.bml.arena[speed]);
            if let Some(Node {
                data: BulletMLNode::Speed { spd_type, spd },
                ..
            }) = &speed_node
            {
                let term = self.get_number_contents(*term, data, runner) as u32;
                let spd = if let Some(SpeedType::Sequence) = spd_type {
                    self.get_number_contents(*spd, data, runner) * f64::from(term)
                        + runner.get_bullet_speed(data.data)
                } else {
                    self.get_speed(*spd_type, *spd, data, runner)
                };
                self.calc_change_speed(spd, term, data, runner);
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
        let term_node =
            RunnerImpl::get_first_child_matching(data.bml, self.act, BulletMLNode::is_term)
                .map(|term| &data.bml.arena[term]);
        if let Some(Node {
            data: BulletMLNode::Term(term),
            ..
        }) = &term_node
        {
            let term = self.get_number_contents(*term, data, runner) as u32;
            let h_node = RunnerImpl::get_first_child_matching(
                data.bml,
                self.act,
                BulletMLNode::is_horizontal,
            )
            .map(|h| &data.bml.arena[h]);
            let v_node =
                RunnerImpl::get_first_child_matching(data.bml, self.act, BulletMLNode::is_vertical)
                    .map(|v| &data.bml.arena[v]);
            if self.bml_type == Some(BulletMLType::Horizontal) {
                if let Some(Node {
                    data: BulletMLNode::Vertical { v_type, v },
                    ..
                }) = &v_node
                {
                    self.accel_x = self.calc_accel_xy(
                        runner.get_bullet_speed_x(),
                        self.get_number_contents(*v, data, runner),
                        term,
                        *v_type,
                    );
                }
                if let Some(Node {
                    data: BulletMLNode::Horizontal { h_type, h },
                    ..
                }) = &h_node
                {
                    self.accel_y = self.calc_accel_xy(
                        runner.get_bullet_speed_y(),
                        self.get_number_contents(*h, data, runner),
                        term,
                        *h_type,
                    );
                }
            } else {
                if let Some(Node {
                    data: BulletMLNode::Horizontal { h_type, h },
                    ..
                }) = &h_node
                {
                    self.accel_x = self.calc_accel_xy(
                        runner.get_bullet_speed_x(),
                        self.get_number_contents(*h, data, runner),
                        term,
                        *h_type,
                    );
                }
                if let Some(Node {
                    data: BulletMLNode::Vertical { v_type, v },
                    ..
                }) = &v_node
                {
                    self.accel_y = self.calc_accel_xy(
                        runner.get_bullet_speed_y(),
                        self.get_number_contents(*v, data, runner),
                        term,
                        *v_type,
                    );
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

    fn run_vanish<D>(&mut self, data: &mut RunnerData<D>, runner: &dyn AppRunner<D>) {
        runner.do_vanish(&mut data.data);
        self.act = None;
    }

    fn get_parameters<D>(&self, data: &mut RunnerData<D>, runner: &dyn AppRunner<D>) -> Parameters {
        let children = self.act.unwrap().children(&data.bml.arena);
        let mut parameters = Vec::new();
        for child in children {
            let child_node = &data.bml.arena[child];
            if let BulletMLNode::Param(expr) = &child_node.data {
                parameters.push(self.get_number_contents(*expr, data, runner));
            }
        }
        parameters
    }

    fn get_number_contents<D>(
        &self,
        expr: fasteval::ExpressionI,
        data: &mut RunnerData<D>,
        runner: &dyn AppRunner<D>,
    ) -> f64 {
        let rank = runner.get_rank(&data.data);
        let expr_ref = expr.from(&data.bml.expr_slab.ps);
        use fasteval::Evaler;
        expr_ref
            .eval(
                &data.bml.expr_slab,
                &mut |name: &str, args: Vec<f64>| match (name, args.as_slice()) {
                    ("v", &[i]) => Some(self.parameters[i as usize - 1]),
                    ("rank", &[]) => Some(rank),
                    ("rand", &[]) => Some(runner.get_rand(data.data)),
                    _ => panic!("Eval {}, {:?}", name, &args),
                },
            )
            .unwrap()
    }
}

#[derive(Debug)]
struct RepeatElem {
    iter: usize,
    end: usize,
    act: NodeId,
}

#[cfg(test)]
mod test_runner {
    use super::{AppRunner, Runner, RunnerData, State};
    use crate::parse::BulletMLParser;
    use crate::tree::{BulletML, BulletMLNode};

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

    impl Drop for TestLog {
        fn drop(&mut self) {
            self.assert_log_end();
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

        fn do_vanish(&self, _data: &mut TestAppData<'a>) {}

        fn do_change_direction(&self, data: &mut TestAppData<'a>, direction: f64) {
            data.logs[self.index]
                .log
                .push(format!("do_change_direction({})", direction));
        }

        fn do_change_speed(&self, data: &mut TestAppData<'a>, speed: f64) {
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
            logs[0].assert_log(r#"Wait(ExpressionI(3))"#, 1);
            for j in 0..100 {
                logs[0].assert_log(&format!(r#"=== {}"#, i * 100 + j + 1), 1);
            }
        }
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
        logs[0].assert_log(r#"Wait(ExpressionI(4))"#, 1);
        for i in 0..300 {
            logs[0].assert_log(&format!(r#"=== {}"#, i + 1), 1);
        }

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
                logs[i].assert_log(r#"Wait(ExpressionI(55))"#, 1);
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
                logs[i].assert_log(r#"Wait(ExpressionI(58))"#, 1);
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
        logs[0].assert_log(r#"Wait(ExpressionI(2))"#, 1);
        logs[0].assert_log(r#"=== 1"#, 1);
        logs[0].assert_log(r#"do_change_speed(0)"#, 1);
        logs[0].assert_log(r#"ChangeSpeed"#, 1);
        logs[0].assert_log(r#"Wait(ExpressionI(5))"#, 1);
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
        logs[0].assert_log(r#"Wait(ExpressionI(3))"#, 1);
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
        logs[1].assert_log(r#"Wait(ExpressionI(6))"#, 1);
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
        logs[2].assert_log(r#"Wait(ExpressionI(6))"#, 1);
        logs[2].assert_log(r#"=== 4"#, 1);
        logs[2].assert_log(r#"=== 5"#, 1);
        logs[2].assert_log(r#"=== 6"#, 1);
        logs[2].assert_log(r#"ChangeSpeed"#, 1);
        for i in 0..60 {
            logs[2].assert_log(&format!(r#"=== {}"#, i + 7), 1);
            logs[2].assert_log(r#"do_change_speed(1)"#, 1);
        }
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
        logs[0].assert_log(r#"Wait(ExpressionI(0))"#, 1);
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
        logs[1].assert_log(r#"Wait(ExpressionI(10))"#, 1);
        logs[1].assert_log(r#"=== 3"#, 1);
        logs[1].assert_log(r#"do_change_direction(90)"#, 1);
        logs[1].assert_log(r#"ChangeDirection"#, 1);
        logs[1].assert_log(r#"Wait(ExpressionI(14))"#, 1);
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
        logs[2].assert_log(r#"Wait(ExpressionI(10))"#, 1);
        logs[2].assert_log(r#"=== 3"#, 1);
        logs[2].assert_log(r#"do_change_direction(-90)"#, 1);
        logs[2].assert_log(r#"ChangeDirection"#, 1);
        logs[2].assert_log(r#"Wait(ExpressionI(14))"#, 1);
        logs[2].assert_log(r#"=== 4"#, 1);
        logs[2].assert_log(r#"do_change_direction(90)"#, 1);
        logs[2].assert_log(r#"Fire(None)"#, 1);
        logs[2].assert_log(r#"Bullet(None)"#, 1);
        logs[2].assert_log(r#"create_simple_bullet(0, 0)"#, 1);
        logs[2].assert_log(r#"Vanish"#, 1);
        logs[2].assert_log(r#"=== 5"#, 1);
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
        logs[0].assert_log(r#"Wait(ExpressionI(0))"#, 1);
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
        logs[1].assert_log(r#"Wait(ExpressionI(12))"#, 1);
        logs[1].assert_log(r#"=== 3"#, 1);
        logs[1].assert_log(r#"do_change_direction(0)"#, 1);
        logs[1].assert_log(r#"ChangeDirection"#, 1);
        logs[1].assert_log(r#"Wait(ExpressionI(16))"#, 1);
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
        logs[2].assert_log(r#"Wait(ExpressionI(12))"#, 1);
        logs[2].assert_log(r#"=== 3"#, 1);
        logs[2].assert_log(r#"do_change_direction(-120)"#, 1);
        logs[2].assert_log(r#"ChangeDirection"#, 1);
        logs[2].assert_log(r#"Wait(ExpressionI(16))"#, 1);
        logs[2].assert_log(r#"=== 4"#, 1);
        logs[2].assert_log(r#"do_change_direction(120)"#, 1);
        logs[2].assert_log(r#"Fire(None)"#, 1);
        logs[2].assert_log(r#"Bullet(None)"#, 1);
        logs[2].assert_log(r#"create_simple_bullet(0, 0.48)"#, 1);
        logs[2].assert_log(r#"Vanish"#, 1);
        logs[2].assert_log(r#"=== 5"#, 1);
    }
}
