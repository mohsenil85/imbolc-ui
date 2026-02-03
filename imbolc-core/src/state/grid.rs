/// Ticks per grid cell based on zoom level.
///
/// Zoom levels map to musical subdivisions at 480 ticks per beat:
/// - 1: 60 ticks (1/8 beat)
/// - 2: 120 ticks (1/4 beat, sixteenth note)
/// - 3: 240 ticks (1/2 beat, eighth note)
/// - 4: 480 ticks (1 beat, quarter note)
/// - 5: 960 ticks (2 beats, half note)
pub fn ticks_per_cell(zoom_level: u8) -> u32 {
    match zoom_level {
        1 => 60,
        2 => 120,
        3 => 240,
        4 => 480,
        5 => 960,
        _ => 240,
    }
}

/// Snap a tick position to the nearest grid boundary.
pub fn snap_to_grid(tick: u32, zoom_level: u8) -> u32 {
    let grid = ticks_per_cell(zoom_level);
    (tick / grid) * grid
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_per_cell_all_levels() {
        assert_eq!(ticks_per_cell(1), 60);
        assert_eq!(ticks_per_cell(2), 120);
        assert_eq!(ticks_per_cell(3), 240);
        assert_eq!(ticks_per_cell(4), 480);
        assert_eq!(ticks_per_cell(5), 960);
    }

    #[test]
    fn ticks_per_cell_default() {
        assert_eq!(ticks_per_cell(0), 240);
        assert_eq!(ticks_per_cell(6), 240);
        assert_eq!(ticks_per_cell(255), 240);
    }

    #[test]
    fn snap_to_grid_aligned() {
        assert_eq!(snap_to_grid(480, 4), 480);
        assert_eq!(snap_to_grid(0, 3), 0);
    }

    #[test]
    fn snap_to_grid_rounds_down() {
        assert_eq!(snap_to_grid(500, 4), 480);
        assert_eq!(snap_to_grid(959, 4), 480);
        assert_eq!(snap_to_grid(100, 3), 0);
        assert_eq!(snap_to_grid(250, 3), 240);
    }
}
