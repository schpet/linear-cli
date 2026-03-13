# notification

> Manage Linear notifications

## Usage

```
Usage:   linear notification

Description:

  Manage Linear notifications

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  

Commands:

  list                       - List notifications            
  count                      - Show unread notification count
  read     <notificationId>  - Mark a notification as read   
  archive  <notificationId>  - Archive a notification
```

## Subcommands

### list

> List notifications

```
Usage:   linear notification list

Description:

  List notifications

Options:

  -h, --help                   - Show this help.                                   
  -w, --workspace     <slug>   - Target workspace (uses credentials)               
  -n, --limit         <limit>  - Maximum number of notifications      (Default: 20)
  --include-archived           - Include archived notifications                    
  --unread                     - Show only unread notifications                    
  -j, --json                   - Output as JSON
```

### count

> Show unread notification count

```
Usage:   linear notification count

Description:

  Show unread notification count

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  
  -j, --json               - Output as JSON
```

### read

> Mark a notification as read

```
Usage:   linear notification read <notificationId>

Description:

  Mark a notification as read

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  
  -j, --json               - Output as JSON
```

### archive

> Archive a notification

```
Usage:   linear notification archive <notificationId>

Description:

  Archive a notification

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  
  -j, --json               - Output as JSON
```
