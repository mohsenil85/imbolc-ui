use serde::{Deserialize, Serialize};

use super::ModuleId;

pub const MAX_CHANNELS: usize = 64;
pub const MAX_BUSES: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputTarget {
    Master,
    Bus(u8), // 1-8
}

impl Default for OutputTarget {
    fn default() -> Self {
        Self::Master
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixerSend {
    pub bus_id: u8,    // 1-8, target bus
    pub level: f32,    // 0.0-1.0
    pub enabled: bool,
}

impl MixerSend {
    pub fn new(bus_id: u8) -> Self {
        Self {
            bus_id,
            level: 0.0,
            enabled: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixerChannel {
    pub id: u8,                       // 1-64
    pub module_id: Option<ModuleId>,  // which OUTPUT module is assigned here
    pub level: f32,                   // 0.0-1.0, default 0.8
    pub pan: f32,                     // -1.0 to 1.0, default 0.0
    pub mute: bool,
    pub solo: bool,
    pub output_target: OutputTarget,
    pub sends: Vec<MixerSend>,       // sends to buses
}

impl MixerChannel {
    pub fn new(id: u8) -> Self {
        let sends = (1..=MAX_BUSES as u8).map(MixerSend::new).collect();
        Self {
            id,
            module_id: None,
            level: 0.8,
            pan: 0.0,
            mute: false,
            solo: false,
            output_target: OutputTarget::Master,
            sends,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.module_id.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixerBus {
    pub id: u8,        // 1-8
    pub name: String,
    pub level: f32,    // 0.0-1.0, default 0.8
    pub pan: f32,      // -1.0 to 1.0, default 0.0
    pub mute: bool,
    pub solo: bool,
}

impl MixerBus {
    pub fn new(id: u8) -> Self {
        Self {
            id,
            name: format!("Bus {}", id),
            level: 0.8,
            pan: 0.0,
            mute: false,
            solo: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MixerSelection {
    Channel(u8),  // 1-64
    Bus(u8),      // 1-8
    Master,
}

impl Default for MixerSelection {
    fn default() -> Self {
        Self::Channel(1)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixerState {
    pub channels: Vec<MixerChannel>,  // 64 channels
    pub buses: Vec<MixerBus>,         // 8 buses
    pub master_level: f32,
    pub master_mute: bool,
    pub selection: MixerSelection,
}

impl MixerState {
    pub fn new() -> Self {
        let channels = (1..=MAX_CHANNELS as u8)
            .map(MixerChannel::new)
            .collect();
        let buses = (1..=MAX_BUSES as u8)
            .map(MixerBus::new)
            .collect();

        Self {
            channels,
            buses,
            master_level: 1.0,
            master_mute: false,
            selection: MixerSelection::default(),
        }
    }

    pub fn channel(&self, id: u8) -> Option<&MixerChannel> {
        self.channels.get((id - 1) as usize)
    }

    pub fn channel_mut(&mut self, id: u8) -> Option<&mut MixerChannel> {
        self.channels.get_mut((id - 1) as usize)
    }

    pub fn bus(&self, id: u8) -> Option<&MixerBus> {
        self.buses.get((id - 1) as usize)
    }

    pub fn bus_mut(&mut self, id: u8) -> Option<&mut MixerBus> {
        self.buses.get_mut((id - 1) as usize)
    }

    /// Find first empty channel (no module assigned)
    pub fn find_free_channel(&self) -> Option<u8> {
        self.channels
            .iter()
            .find(|ch| ch.is_empty())
            .map(|ch| ch.id)
    }

    /// Assign a module to a channel
    pub fn assign_module(&mut self, channel_id: u8, module_id: ModuleId) -> bool {
        if let Some(ch) = self.channel_mut(channel_id) {
            ch.module_id = Some(module_id);
            true
        } else {
            false
        }
    }

    /// Unassign a module from its channel (when module is deleted)
    pub fn unassign_module(&mut self, module_id: ModuleId) {
        for ch in &mut self.channels {
            if ch.module_id == Some(module_id) {
                ch.module_id = None;
                break;
            }
        }
    }

    /// Find which channel a module is assigned to
    pub fn find_channel_for_module(&self, module_id: ModuleId) -> Option<u8> {
        self.channels
            .iter()
            .find(|ch| ch.module_id == Some(module_id))
            .map(|ch| ch.id)
    }

    /// Get channels that have modules assigned (for display)
    pub fn active_channels(&self) -> impl Iterator<Item = &MixerChannel> {
        self.channels.iter().filter(|ch| ch.module_id.is_some())
    }

    /// Check if any channel is soloed
    pub fn any_solo(&self) -> bool {
        self.channels.iter().any(|ch| ch.solo) || self.buses.iter().any(|b| b.solo)
    }

    /// Check if any channel (not bus) is soloed
    pub fn any_channel_solo(&self) -> bool {
        self.channels.iter().any(|ch| ch.solo)
    }

    /// Check if any bus is soloed
    pub fn any_bus_solo(&self) -> bool {
        self.buses.iter().any(|b| b.solo)
    }

    /// Compute effective mute for a channel, considering solo state
    pub fn effective_channel_mute(&self, ch: &MixerChannel) -> bool {
        if self.any_channel_solo() {
            !ch.solo
        } else {
            ch.mute || self.master_mute
        }
    }

    /// Compute effective mute for a bus, considering solo state
    pub fn effective_bus_mute(&self, bus: &MixerBus) -> bool {
        if self.any_bus_solo() {
            !bus.solo
        } else {
            bus.mute
        }
    }

    /// Collect audio updates for all channels (module_id, level, mute)
    pub fn collect_channel_updates(&self) -> Vec<(ModuleId, f32, bool)> {
        self.channels
            .iter()
            .filter_map(|ch| {
                ch.module_id.map(|mid| {
                    (mid, ch.level * self.master_level, self.effective_channel_mute(ch))
                })
            })
            .collect()
    }

    /// Move selection left/right
    pub fn move_selection(&mut self, delta: i8) {
        self.selection = match self.selection {
            MixerSelection::Channel(id) => {
                let new_id = (id as i8 + delta).clamp(1, MAX_CHANNELS as i8) as u8;
                MixerSelection::Channel(new_id)
            }
            MixerSelection::Bus(id) => {
                let new_id = (id as i8 + delta).clamp(1, MAX_BUSES as i8) as u8;
                MixerSelection::Bus(new_id)
            }
            MixerSelection::Master => MixerSelection::Master,
        };
    }

    /// Jump to first (1) or last (-1) in current section
    pub fn jump_selection(&mut self, direction: i8) {
        self.selection = match self.selection {
            MixerSelection::Channel(_) => {
                if direction > 0 {
                    MixerSelection::Channel(1)
                } else {
                    MixerSelection::Channel(MAX_CHANNELS as u8)
                }
            }
            MixerSelection::Bus(_) => {
                if direction > 0 {
                    MixerSelection::Bus(1)
                } else {
                    MixerSelection::Bus(MAX_BUSES as u8)
                }
            }
            MixerSelection::Master => MixerSelection::Master,
        };
    }

    /// Cycle between channel/bus/master sections
    pub fn cycle_section(&mut self) {
        self.selection = match self.selection {
            MixerSelection::Channel(_) => MixerSelection::Bus(1),
            MixerSelection::Bus(_) => MixerSelection::Master,
            MixerSelection::Master => MixerSelection::Channel(1),
        };
    }

    /// Cycle output target for the selected channel (channels only)
    pub fn cycle_output(&mut self) {
        if let MixerSelection::Channel(id) = self.selection {
            if let Some(ch) = self.channel_mut(id) {
                ch.output_target = match ch.output_target {
                    OutputTarget::Master => OutputTarget::Bus(1),
                    OutputTarget::Bus(n) if n < MAX_BUSES as u8 => OutputTarget::Bus(n + 1),
                    OutputTarget::Bus(_) => OutputTarget::Master,
                };
            }
        }
    }

    /// Cycle output target backwards for the selected channel (channels only)
    pub fn cycle_output_reverse(&mut self) {
        if let MixerSelection::Channel(id) = self.selection {
            if let Some(ch) = self.channel_mut(id) {
                ch.output_target = match ch.output_target {
                    OutputTarget::Master => OutputTarget::Bus(MAX_BUSES as u8),
                    OutputTarget::Bus(1) => OutputTarget::Master,
                    OutputTarget::Bus(n) => OutputTarget::Bus(n - 1),
                };
            }
        }
    }
}

impl Default for MixerState {
    fn default() -> Self {
        Self::new()
    }
}
