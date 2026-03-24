pub fn bubble_sort<T: Ord>(arr: &mut [T]) {
    let len = arr.len();
    for i in 0..len {
        for j in 0..len - 1 - i {
            if arr[j] > arr[j + 1] {
                arr.swap(j, j + 1);
            }
        }
    }
}

#[test]
fn test_bubble_sort() {
    let mut arr = vec![3, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5];
    bubble_sort(&mut arr);
    assert_eq!(arr, vec![1, 1, 2, 3, 3, 4, 5, 5, 5, 6, 9]);
}

#[test]
fn test_bubble_sort_empty() {
    let mut arr: Vec<i32> = vec![];
    bubble_sort(&mut arr);
    assert_eq!(arr, vec![]);
}

#[test]
fn test_bubble_sort_single() {
    let mut arr = vec![1];
    bubble_sort(&mut arr);
    assert_eq!(arr, vec![1]);
}

#[test]
fn test_bubble_sort_already_sorted() {
    let mut arr = vec![1, 2, 3, 4, 5];
    bubble_sort(&mut arr);
    assert_eq!(arr, vec![1, 2, 3, 4, 5]);
}
