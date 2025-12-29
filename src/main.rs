fn print_commits() -> Result<(), anyhow::Error> {
    let repo = gix::open(".")?;
    let head_commit = repo.head_commit()?;
    let msg = head_commit.message()?;
    let id = head_commit.id().shorten_or_id();
    let title = msg.title.to_string();
    println!("Commit {} {}", id, title.trim());

    let budget = 10;
    let mut commit = head_commit;

    for _ in 0..budget {
        // TODO support multiple parent IDs
        let Some(parent_id) = commit.parent_ids().next() else {
            // No parent left
            break;
        };
        commit = repo.find_commit(parent_id)?;
        let msg = commit.message()?;
        let id = commit.id().shorten_or_id();
        let title = msg.title.to_string();
        println!("Commit {} {}", id, title.trim());
    }
    Ok(())
}

fn main() -> Result<(), anyhow::Error> {
    print_commits()?;
    Ok(())
}
