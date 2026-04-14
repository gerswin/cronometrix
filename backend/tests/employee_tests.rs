mod common;

#[tokio::test]
#[ignore = "Requires employees module from Plan 01-03"]
async fn crud_employee_endpoints() {
    // Will test: POST creates, GET lists, GET/:id returns, PATCH updates, DELETE soft-deletes
    todo!("Implement after Plan 01-03 delivers employee handlers");
}

#[tokio::test]
#[ignore = "Requires employees module from Plan 01-03"]
async fn soft_delete_only_no_hard_delete() {
    // Will test: DELETE sets status=inactive and deleted_at, row still exists
    todo!("Implement after Plan 01-03 delivers deactivate_employee");
}

#[tokio::test]
#[ignore = "Requires employees module from Plan 01-03"]
async fn employee_search_and_filter() {
    // Will test: GET /employees?name=X&department_id=Y&status=active works
    todo!("Implement after Plan 01-03 delivers list_employees with filters");
}

#[tokio::test]
#[ignore = "Requires employees module from Plan 01-03"]
async fn employee_department_constraint() {
    // Will test: Creating employee with non-existent department_id returns error
    todo!("Implement after Plan 01-03 delivers FK validation");
}
