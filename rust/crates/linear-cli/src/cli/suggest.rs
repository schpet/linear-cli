pub(crate) fn closest<'a>(word: &str, choices: &'a [&str]) -> Option<&'a str> {
    let word = word.to_lowercase();
    let mut best = None;
    for candidate in choices {
        let candidate_distance = distance(&word, &candidate.to_lowercase());
        if best.is_none_or(|(_, best_distance)| candidate_distance < best_distance) {
            best = Some((*candidate, candidate_distance));
        }
    }
    best.map(|(candidate, _)| candidate)
}

fn distance(left: &str, right: &str) -> usize {
    let width = right.chars().count();
    let mut previous = (0..=width).collect::<Vec<_>>();
    let mut result = width;
    for (row, letter) in left.chars().enumerate() {
        let mut left_cost = row + 1;
        let mut current = vec![left_cost];
        for ((diagonal, above), other) in previous
            .iter()
            .copied()
            .zip(previous.iter().skip(1).copied())
            .zip(right.chars())
        {
            let cost = (above + 1)
                .min(left_cost + 1)
                .min(diagonal + usize::from(letter != other));
            current.push(cost);
            left_cost = cost;
        }
        result = left_cost;
        previous = current;
    }
    result
}
