use indextree::{Arena, NodeId};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
pub enum BulletMLExpression {
    Const(f64),
    Expr(fasteval::ExpressionI),
}

#[derive(Debug)]
pub enum BulletMLNode {
    BulletML {
        bml_type: Option<BulletMLType>,
    },

    Bullet(Option<String>),
    Action(Option<String>),
    Fire(Option<String>),

    ChangeDirection,
    ChangeSpeed,

    Accel,

    Wait(BulletMLExpression),

    Vanish,

    Repeat,

    Direction {
        dir_type: Option<DirectionType>,
        dir: BulletMLExpression,
    },

    Speed {
        spd_type: Option<SpeedType>,
        spd: BulletMLExpression,
    },

    Horizontal {
        h_type: HVType,
        h: BulletMLExpression,
    },
    Vertical {
        v_type: HVType,
        v: BulletMLExpression,
    },

    Term(BulletMLExpression),

    Times(BulletMLExpression),

    BulletRef(String),
    ActionRef(String),
    FireRef(String),

    Param(BulletMLExpression),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum BulletMLType {
    Vertical,
    Horizontal,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum DirectionType {
    Aim,
    Absolute,
    Relative,
    Sequence,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SpeedType {
    Absolute,
    Relative,
    Sequence,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum HVType {
    Absolute,
    Relative,
    Sequence,
}

impl BulletMLNode {
    pub fn is_top_action(&self) -> bool {
        if let BulletMLNode::Action(Some(label)) = self {
            label.starts_with("top")
        } else {
            false
        }
    }

    pub fn match_any_action(&self) -> Option<()> {
        match self {
            BulletMLNode::Action { .. } | BulletMLNode::ActionRef(..) => Some(()),
            _ => None,
        }
    }

    pub fn match_any_bullet(&self) -> Option<()> {
        match self {
            BulletMLNode::Bullet { .. } | BulletMLNode::BulletRef(..) => Some(()),
            _ => None,
        }
    }

    pub fn match_direction(&self) -> Option<(Option<DirectionType>, BulletMLExpression)> {
        if let BulletMLNode::Direction { dir_type, dir } = self {
            Some((*dir_type, *dir))
        } else {
            None
        }
    }

    pub fn match_speed(&self) -> Option<(Option<SpeedType>, BulletMLExpression)> {
        if let BulletMLNode::Speed { spd_type, spd } = self {
            Some((*spd_type, *spd))
        } else {
            None
        }
    }

    pub fn match_horizontal(&self) -> Option<(HVType, BulletMLExpression)> {
        if let BulletMLNode::Horizontal { h_type, h } = self {
            Some((*h_type, *h))
        } else {
            None
        }
    }

    pub fn match_vertical(&self) -> Option<(HVType, BulletMLExpression)> {
        if let BulletMLNode::Vertical { v_type, v } = self {
            Some((*v_type, *v))
        } else {
            None
        }
    }

    pub fn match_term(&self) -> Option<BulletMLExpression> {
        if let BulletMLNode::Term(term) = self {
            Some(*term)
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct BulletML {
    pub arena: Arena<BulletMLNode>,
    pub root: NodeId,
    pub bullet_refs: HashMap<String, NodeId>,
    pub action_refs: HashMap<String, NodeId>,
    pub fire_refs: HashMap<String, NodeId>,
    pub expr_slab: fasteval::Slab,
}
