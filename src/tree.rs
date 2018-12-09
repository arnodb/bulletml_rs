use indextree::{Arena, NodeId};
use meval::Expr;
use std::collections::HashMap;

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

    Wait(Expr),

    Vanish,

    Repeat,

    Direction {
        dir_type: Option<DirectionType>,
        dir: Expr,
    },

    Speed {
        spd_type: Option<SpeedType>,
        spd: Expr,
    },

    Horizontal {
        h_type: HVType,
        h: Expr,
    },
    Vertical {
        v_type: HVType,
        v: Expr,
    },

    Term(Expr),

    Times(Expr),

    BulletRef(String),
    ActionRef(String),
    FireRef(String),

    Param(Expr),
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
    pub fn is_action(&self) -> bool {
        if let BulletMLNode::Action { .. } = self {
            true
        } else {
            false
        }
    }

    pub fn is_action_ref(&self) -> bool {
        if let BulletMLNode::ActionRef(..) = self {
            true
        } else {
            false
        }
    }

    pub fn is_top_action(&self) -> bool {
        if let BulletMLNode::Action(Some(label)) = self {
            label.starts_with("top")
        } else {
            false
        }
    }

    pub fn is_bullet(&self) -> bool {
        if let BulletMLNode::Bullet { .. } = self {
            true
        } else {
            false
        }
    }

    pub fn is_bullet_ref(&self) -> bool {
        if let BulletMLNode::BulletRef(..) = self {
            true
        } else {
            false
        }
    }

    pub fn is_direction(&self) -> bool {
        if let BulletMLNode::Direction { .. } = self {
            true
        } else {
            false
        }
    }

    pub fn is_speed(&self) -> bool {
        if let BulletMLNode::Speed { .. } = self {
            true
        } else {
            false
        }
    }

    pub fn is_horizontal(&self) -> bool {
        if let BulletMLNode::Horizontal { .. } = self {
            true
        } else {
            false
        }
    }

    pub fn is_vertical(&self) -> bool {
        if let BulletMLNode::Vertical { .. } = self {
            true
        } else {
            false
        }
    }

    pub fn is_term(&self) -> bool {
        if let BulletMLNode::Term(..) = self {
            true
        } else {
            false
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
}
