fn print_commits() -> Result<(), anyhow::Error> {
    let repo = gix::open(".")?;
    let head_commit = repo.head_commit()?;
    let msg = head_commit.message()?;
    let id = head_commit.id().shorten_or_id();
    println!("Commit {} {}", id, msg.title);

    let mut parent_ids = head_commit.parent_ids();
    while let Some(parent_id) = parent_ids.next() {
        let cmt = repo.find_commit(parent_id)?;
        let msg = cmt.message()?;
        let id = cmt.id().shorten_or_id();
        println!("Commit {} {}", id, msg.title);
    }
    Ok(())
}

fn main() -> Result<(), anyhow::Error> {
    print_commits()?;
    Ok(())
}
