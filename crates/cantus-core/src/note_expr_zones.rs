//! **MPE zones** (MIDI Polyphonic Expression 1.1, §2): how an MPE controller's channels are
//! laid out. A *lower zone* has its master on channel 1 and its members on 2, 3, …; an
//! *upper zone* its master on 16 and its members on 15, 14, …; a controller uses one zone
//! or both, their members never overlapping. Each note plays alone on a member channel,
//! whose pitch bend, channel pressure and CC 74 are that note's; the master channel's
//! messages are the whole zone's, as on any keyboard.
//!
//! The *MPE Configuration Message* (MCM) — RPN 6 on a zone's master channel, its data the
//! number of member channels (0 turns the zone off) — sets a zone; RPN 0 (pitch bend
//! sensitivity) on a member channel sets the members' bend range (48 semitones unless
//! told), on the master the master's (2). Channels here count from 0.

use serde::{Deserialize, Serialize};

/// One zone: its member channels and the bend ranges.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Zone {
    /// How many member channels (1…15).
    pub members: u8,
    /// Semitones a member channel's full bend reaches (MPE's default 48).
    #[serde(default = "default_member_bend")]
    pub member_bend: u8,
    /// Semitones the master channel's full bend reaches (MPE's default 2), as the controller
    /// says it. Kept, not applied: the manager channel's bend goes the ordinary way, through
    /// the instrument's own bend range, as any keyboard's does (review fix 8).
    #[serde(default = "default_master_bend")]
    pub master_bend: u8,
}

fn default_member_bend() -> u8 {
    48
}

fn default_master_bend() -> u8 {
    2
}

impl Zone {
    /// A zone of `members` channels with MPE's default bend ranges.
    pub fn new(members: u8) -> Zone {
        Zone {
            members: members.clamp(1, 15),
            member_bend: 48,
            master_bend: 2,
        }
    }
}

/// The zone layout of the MPE input (and of what Cantus sends as MPE).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MpeZones {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lower: Option<Zone>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upper: Option<Zone>,
}

/// What a channel is in a layout.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Role {
    /// Not in any zone: an ordinary channel.
    None,
    /// A zone's master (`upper` tells which zone).
    Master { upper: bool },
    /// A member of a zone.
    Member { upper: bool },
}

impl MpeZones {
    /// The usual layout of an MPE controller: a lower zone of 15 members.
    pub fn standard() -> MpeZones {
        MpeZones {
            lower: Some(Zone::new(15)),
            upper: None,
        }
    }

    pub fn is_on(&self) -> bool {
        self.lower.is_some() || self.upper.is_some()
    }

    /// The layout made consistent: members within 1…15, an upper zone shrunk where it
    /// would overlap the lower one (the lower keeps what it has, as MPE §2.3 says of the
    /// zone set last — the caller applies the newer one last).
    pub fn repaired(mut self) -> MpeZones {
        if let Some(z) = self.lower.as_mut() {
            z.members = z.members.clamp(1, 15);
        }
        if let Some(z) = self.upper.as_mut() {
            z.members = z.members.clamp(1, 15);
        }
        if let (Some(l), Some(u)) = (self.lower, self.upper.as_mut()) {
            // Lower members 1..=l, upper members 14 down to 15 - u: they overlap when
            // l + u > 14.
            let room = 14u8.saturating_sub(l.members);
            if room == 0 {
                self.upper = None;
            } else {
                u.members = u.members.min(room);
            }
        }
        self
    }

    /// What channel `ch` (0…15) is.
    pub fn role(&self, ch: u8) -> Role {
        let ch = ch & 15;
        if let Some(l) = self.lower {
            if ch == 0 {
                return Role::Master { upper: false };
            }
            if ch <= l.members {
                return Role::Member { upper: false };
            }
        }
        if let Some(u) = self.upper {
            if ch == 15 {
                return Role::Master { upper: true };
            }
            if ch >= 15 - u.members {
                return Role::Member { upper: true };
            }
        }
        Role::None
    }

    /// The zone of a role.
    pub fn zone(&self, upper: bool) -> Option<Zone> {
        if upper {
            self.upper
        } else {
            self.lower
        }
    }

    /// The layout after an MCM: `members` channels for the zone whose master is `master`
    /// (0 lower, 15 upper; any other channel is not an MCM), 0 turning it off. The zone set
    /// last wins an overlap (MPE §2.3); bend ranges go back to their defaults (§2.4).
    pub fn after_mcm(self, master: u8, members: u8) -> MpeZones {
        let zone = (members > 0).then(|| Zone::new(members.min(15)));
        let mut z = self;
        match master & 15 {
            0 => {
                z.lower = zone;
                if let (Some(l), Some(u)) = (z.lower, z.upper.as_mut()) {
                    let room = 14u8.saturating_sub(l.members);
                    if room == 0 {
                        z.upper = None;
                    } else {
                        u.members = u.members.min(room);
                    }
                }
            }
            15 => {
                z.upper = zone;
                if let (Some(u), Some(l)) = (z.upper, z.lower.as_mut()) {
                    let room = 14u8.saturating_sub(u.members);
                    if room == 0 {
                        z.lower = None;
                    } else {
                        l.members = l.members.min(room);
                    }
                }
            }
            _ => {}
        }
        z
    }

    /// The member channels of a zone, in the order a sender hands them out (lower: 1, 2, …;
    /// upper: 14, 13, …). Empty when the zone is off.
    pub fn members(&self, upper: bool) -> impl Iterator<Item = u8> {
        let n = self.zone(upper).map_or(0, |z| z.members);
        (0..n).map(move |i| if upper { 14 - i } else { 1 + i })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_follow_the_layout_and_mcms_resolve_overlaps() {
        let z = MpeZones::standard();
        assert_eq!(z.role(0), Role::Master { upper: false });
        assert_eq!(z.role(1), Role::Member { upper: false });
        assert_eq!(z.role(15), Role::Member { upper: false });
        assert_eq!(MpeZones::default().role(3), Role::None);
        // Split: lower 7 members (1–7), upper 7 (14–8).
        let s = MpeZones::default().after_mcm(0, 7).after_mcm(15, 7);
        assert_eq!(s.role(7), Role::Member { upper: false });
        assert_eq!(s.role(8), Role::Member { upper: true });
        assert_eq!(s.role(15), Role::Master { upper: true });
        assert_eq!(
            s.members(true).collect::<Vec<_>>(),
            (8..=14).rev().collect::<Vec<_>>()
        );
        // The upper set to 10 after: the lower shrinks to 4.
        let s = s.after_mcm(15, 10);
        assert_eq!(s.lower.unwrap().members, 4);
        // Zero members turn a zone off.
        assert_eq!(s.after_mcm(15, 0).upper, None);
        assert_eq!(
            MpeZones::standard().after_mcm(3, 5),
            MpeZones::standard(),
            "not a master"
        );
        let r = MpeZones {
            lower: Some(Zone::new(15)),
            upper: Some(Zone::new(3)),
        }
        .repaired();
        assert_eq!(r.upper, None);
    }
}
