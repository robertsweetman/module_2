# MCP server execution

In this folder we need a .env file with the DATABASE_URL variable to connect to the tenders database

make sure nodejs and npm are installed `sudo apt install nodejs npm`

```
cd mcp-server
npm install
npm start
```

In the IDE MCP settings you need something like this also

```json
{
  "mcpServers": {
    "irish-tenders": {
      "command": "node",
      "args": ["index.js"],
      "cwd": "/mnt/c/Users/rober/GitHub/module_2/mcp-server"
    }
  }
}
```

In VSCode (rather than something like Cursor) use because you need to tell the process to run inside WSL

```json
{
	"servers": {
		"irish-tenders": {
			"command": "wsl",
			"args": ["node", "index.js"],
			"cwd": "C:\\Users\\rober\\GitHub\\module_2\\mcp-server"
		}
	},
	"inputs": []
}
```

This allows users/data analysts to ask questions of the entire corpus of tender data without having to think up manual queries and so on. 
