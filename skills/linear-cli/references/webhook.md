# webhook

> Manage Linear webhooks

## Usage

```
Usage:   linear webhook

Description:

  Manage Linear webhooks

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  

Commands:

  list                 - List webhooks   
  view    <webhookId>  - View a webhook  
  create               - Create a webhook
  update  <webhookId>  - Update a webhook
  delete  <webhookId>  - Delete a webhook
```

## Subcommands

### list

> List webhooks

```
Usage:   linear webhook list

Description:

  List webhooks

Options:

  -h, --help                     - Show this help.                                   
  -w, --workspace     <slug>     - Target workspace (uses credentials)               
  -n, --limit         <limit>    - Maximum number of webhooks           (Default: 20)
  --team              <teamKey>  - Filter by team key                                
  --include-archived             - Include archived webhooks                         
  -j, --json                     - Output as JSON
```

### view

> View a webhook

```
Usage:   linear webhook view <webhookId>

Description:

  View a webhook

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  
  -j, --json               - Output as JSON
```

### create

> Create a webhook

```
Usage:   linear webhook create

Description:

  Create a webhook

Options:

  -h, --help                             - Show this help.                                                     
  -w, --workspace       <slug>           - Target workspace (uses credentials)                                 
  -u, --url             <url>            - Webhook URL (required)                                              
  -r, --resource-types  <resourceTypes>  - Comma-separated resource types (e.g. Issue,Comment)                 
  -l, --label           <label>          - Webhook label                                                       
  -t, --team            <teamKey>        - Team key (defaults to current team)                                 
  --all-public-teams                     - Enable the webhook for all public teams instead of a specific team  
  --secret              <secret>         - Secret used to sign webhook payloads                                
  --disabled                             - Create the webhook disabled                                         
  -j, --json                             - Output as JSON
```

### update

> Update a webhook

```
Usage:   linear webhook update <webhookId>

Description:

  Update a webhook

Options:

  -h, --help                             - Show this help.                      
  -w, --workspace       <slug>           - Target workspace (uses credentials)  
  -u, --url             <url>            - New webhook URL                      
  -r, --resource-types  <resourceTypes>  - New comma-separated resource types   
  -l, --label           <label>          - New webhook label                    
  --secret              <secret>         - New secret used to sign payloads     
  --enabled                              - Enable the webhook                   
  --disabled                             - Disable the webhook                  
  -j, --json                             - Output as JSON
```

### delete

> Delete a webhook

```
Usage:   linear webhook delete <webhookId>

Description:

  Delete a webhook

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  
  -y, --yes                - Skip confirmation prompt             
  -j, --json               - Output as JSON
```
