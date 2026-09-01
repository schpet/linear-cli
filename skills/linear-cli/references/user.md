# user

> Manage Linear users

## Usage

```
Usage:   linear user

Description:

  Manage Linear users

Options:

  -h, --help           - Show this help.                      
  --workspace  <slug>  - Target workspace (uses credentials)  

Commands:

  list  - List members of the workspace
```

## Subcommands

### list

> List members of the workspace

```
Usage:   linear user list

Description:

  List members of the workspace

Options:

  -h, --help           - Show this help.                                                               
  --workspace  <slug>  - Target workspace (uses credentials)                                           
  -a, --all            - Include inactive members                                                      
  -j, --json           - Output as JSON; a member's url mentions them when pasted into Markdown. This  
                         searches the whole workspace — prefer `linear team members <TEAM>`, and       
                         confirm before mentioning someone outside the team
```
