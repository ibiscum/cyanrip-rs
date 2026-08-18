pub fn frames_to_cue(frames: u32) -> String {
    let min = frames / (75 * 60);
    let sec = (frames - (min * 75 * 60)) / 75;
    let left = frames - (min * 75 * 60) - (sec * 75);
    format!("{min:02}:{sec:02}:{left:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cue_time_conversion() {
        assert_eq!(frames_to_cue(0), "00:00:00");
        assert_eq!(frames_to_cue(75), "00:01:00");
        assert_eq!(frames_to_cue(4500), "01:00:00");
    }
}
