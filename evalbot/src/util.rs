use std;

pub fn wrap_and_trim_output(input: &str, max_len: usize) -> Vec<&str> {
    let mut ret = vec![];
    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.len() <= max_len {
            ret.push(line);
        } else {
            let mut leftover = line;
            while let Some((idx, _)) = leftover.char_indices().nth(max_len) {
                let (part, after) = leftover.split_at(idx);
                ret.push(part);
                leftover = after;
            }
            if leftover.len() > 0 {
                ret.push(leftover);
            }
        }
    }
    ret
}

pub fn truncate_output(mut input: Vec<&str>, max_lines: usize) -> (bool, Vec<&str>) {
    let max_lines = std::cmp::min(max_lines, input.len());
    if max_lines < input.len() {
        input.truncate(max_lines);
        (true, input)
    } else {
        (false, input)
    }
}
