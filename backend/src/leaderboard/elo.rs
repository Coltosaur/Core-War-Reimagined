const K_FACTOR: f64 = 32.0;

fn expected_score(rating: i32, opponent_rating: i32) -> f64 {
    1.0 / (1.0 + 10.0_f64.powf((opponent_rating - rating) as f64 / 400.0))
}

pub fn rating_change(winner_rating: i32, loser_rating: i32) -> (i32, i32) {
    let e_winner = expected_score(winner_rating, loser_rating);
    let delta = (K_FACTOR * (1.0 - e_winner)).round() as i32;
    (winner_rating + delta, loser_rating - delta)
}

pub fn tie_change(rating_a: i32, rating_b: i32) -> (i32, i32) {
    let e_a = expected_score(rating_a, rating_b);
    let delta_a = (K_FACTOR * (0.5 - e_a)).round() as i32;
    (rating_a + delta_a, rating_b - delta_a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_ratings_winner_gains() {
        let (w, l) = rating_change(1200, 1200);
        assert!(w > 1200);
        assert!(l < 1200);
        assert_eq!(w - 1200, 1200 - l);
    }

    #[test]
    fn higher_rated_gains_less() {
        let (w_high, _) = rating_change(1600, 1200);
        let (w_equal, _) = rating_change(1200, 1200);
        assert!(w_high - 1600 < w_equal - 1200);
    }

    #[test]
    fn upset_yields_larger_swing() {
        let (w, _) = rating_change(1200, 1600);
        assert!(w - 1200 > 16);
    }

    #[test]
    fn equal_ratings_tie_no_change() {
        let (a, b) = tie_change(1200, 1200);
        assert_eq!(a, 1200);
        assert_eq!(b, 1200);
    }

    #[test]
    fn tie_favors_lower_rated() {
        let (a, b) = tie_change(1600, 1200);
        assert!(a < 1600);
        assert!(b > 1200);
    }

    #[test]
    fn rating_changes_are_zero_sum() {
        let (w, l) = rating_change(1500, 1300);
        assert_eq!((w + l), (1500 + 1300));

        let (a, b) = tie_change(1400, 1100);
        assert_eq!((a + b), (1400 + 1100));
    }
}
