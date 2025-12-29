fn print_commits() -> Result<(), anyhow::Error> {
    let repo = gix::open(".")?;
    let head_commit = repo.head_commit()?;
    let msg = head_commit.message()?;
    println!("head commit msg title: {}", msg.title);
    Ok(())
}

fn main() -> Result<(), anyhow::Error> {
    print_commits()?;
    Ok(())
}
