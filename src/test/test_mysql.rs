use mysql::*;
use mysql::prelude::*;
use std::env::args;

#[derive(Debug, PartialEq, Eq)]
struct Payment {
    customer_id: i32,
    amount: i32,
    account_name: Option<String>,
}
fn main()
{
    let mut pwd = "as147258369".to_string();

    args().enumerate().for_each(|it|{
        if it.0 == 1{
            pwd = it.1;
        }
    });

    let url = format!("mysql://root:{}@localhost:3306/sql_test",pwd);

    let pool = Pool::new(url).unwrap();

    let mut conn = pool.get_conn().unwrap();

// Let's create a table for payments.
    conn.query_drop(
        r"CREATE TEMPORARY TABLE payment (
        customer_id int not null,
        amount int not null,
        account_name text
    )").unwrap();

    let payments = vec![
        Payment { customer_id: 1, amount: 2, account_name: None },
        Payment { customer_id: 3, amount: 4, account_name: Some("foo".into()) },
        Payment { customer_id: 5, amount: 6, account_name: None },
        Payment { customer_id: 7, amount: 8, account_name: None },
        Payment { customer_id: 9, amount: 10, account_name: Some("bar".into()) },
    ];

// Now let's insert payments to the database
    conn.exec_batch(
        r"INSERT INTO payment (customer_id, amount, account_name)
      VALUES (:customer_id, :amount, :account_name)",
        payments.iter().map(|p| params! {
        "customer_id" => p.customer_id,
        "amount" => p.amount,
        "account_name" => &p.account_name,
    })
    ).unwrap();

// Let's select payments from database. Type inference should do the trick here.
    let selected_payments = conn
        .query_map(
            "SELECT customer_id, amount, account_name from payment",
            |(customer_id, amount, account_name)| {
                Payment { customer_id, amount, account_name }
            },
        ).unwrap();

// Let's make sure, that `payments` equals to `selected_payments`.
// Mysql gives no guaranties on order of returned rows
// without `ORDER BY`, so assume we are lucky.
    dbg!(selected_payments);
    println!("Yay!");
}