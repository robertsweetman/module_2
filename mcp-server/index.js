#!/usr/bin/env node

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import pg from "pg";
import { z } from "zod";
import dotenv from "dotenv";
import fs from "fs";

// Load environment variables
dotenv.config();

const { Pool } = pg;

// ------------------------------------------
// Optional SSL configuration for RDS / remote DB
// ------------------------------------------
let sslConfig;

// Enable SSL when explicitly requested *or* when the connection string
// already signals it (e.g., ?sslmode=require).
if (
  process.env.DB_SSL === "true" ||
  (process.env.DATABASE_URL && process.env.DATABASE_URL.includes("sslmode=require"))
) {
  if (process.env.DB_SSL_CA_PATH) {
    // Use a custom CA bundle if provided (typical for AWS RDS).
    try {
      sslConfig = {
        ca: fs.readFileSync(process.env.DB_SSL_CA_PATH).toString(),
        rejectUnauthorized: true,
      };
    } catch (err) {
      console.warn("⚠️  Could not read CA certificate at", process.env.DB_SSL_CA_PATH, "– falling back to insecure mode:", err.message);
      sslConfig = { rejectUnauthorized: false };
    }
  } else {
    // Fallback: use SSL but skip CA validation (okay for dev / quick fixes).
    sslConfig = { rejectUnauthorized: false };
  }
}

// Build pool configuration dynamically so non-SSL setups remain untouched.
const poolConfig = {
  connectionString: process.env.DATABASE_URL,
  max: 10,
  idleTimeoutMillis: 30000,
  connectionTimeoutMillis: 2000,
};

if (sslConfig) {
  poolConfig.ssl = sslConfig;
}

// Create PostgreSQL connection pool
const pool = new Pool(poolConfig);

// Test database connection on startup
async function testConnection() {
  try {
    const client = await pool.connect();
    await client.query('SELECT NOW()');
    client.release();
    console.log('✅ Database connection successful');
  } catch (err) {
    console.error('❌ Database connection failed:', err);
    process.exit(1);
  }
}

// Create MCP server
const server = new McpServer({
  name: "irish-tenders-server",
  version: "1.0.0"
});

// ==========================================
// RESOURCES - Data endpoints for LLMs
// ==========================================

// Individual tender resource
server.registerResource(
  "tender",
  "tender://",
  {
    title: "Tender Record",
    description: "Individual tender record by resource_id"
  },
  async (uri, { resource_id }) => {
    const client = await pool.connect();
    try {
      const result = await client.query(
        'SELECT * FROM tender_records WHERE resource_id = $1',
        [resource_id]
      );
      
      if (result.rows.length === 0) {
        return {
          contents: [{
            uri: uri.href,
            text: `No tender found with resource_id: ${resource_id}`
          }]
        };
      }

      const tender = result.rows[0];
      return {
        contents: [{
          uri: uri.href,
          text: `# Tender: ${tender.title}

**Resource ID:** ${tender.resource_id}
**Contracting Authority:** ${tender.ca}
**Published:** ${tender.published}
**Deadline:** ${tender.deadline}
**Status:** ${tender.status}
**Procedure:** ${tender.procedure}
**Value:** ${tender.value}
**Award Date:** ${tender.awarddate}
**PDF URL:** ${tender.pdf_url}

**Additional Info:** ${tender.info}

**Created:** ${tender.created_at}
**Cycle:** ${tender.cycle}`
        }]
      };
    } finally {
      client.release();
    }
  }
);

// Tender search results resource
server.registerResource(
  "search",
  "search://",
  {
    title: "Tender Search Results",
    description: "Search results for tender records"
  },
  async (uri, { query }) => {
    const client = await pool.connect();
    try {
      const result = await client.query(
        `SELECT resource_id, title, ca, published, deadline, status, value 
         FROM tender_records 
         WHERE title ILIKE $1 OR ca ILIKE $1 OR info ILIKE $1
         ORDER BY published DESC 
         LIMIT 20`,
        [`%${query}%`]
      );

      const searchResults = result.rows.map(row => 
        `- **${row.title}** (${row.resource_id})
  - Authority: ${row.ca}
  - Published: ${row.published}
  - Deadline: ${row.deadline}
  - Status: ${row.status}
  - Value: ${row.value}`
      ).join('\n\n');

      return {
        contents: [{
          uri: uri.href,
          text: `# Search Results for "${query}"

Found ${result.rows.length} tenders:

${searchResults || 'No results found'}`
        }]
      };
    } finally {
      client.release();
    }
  }
);

// ==========================================
// TOOLS - Actions the LLM can execute
// ==========================================

// Search tenders tool
server.registerTool(
  "search_tenders",
  {
    title: "Search Tenders",
    description: "Search for tenders by title, contracting authority, or description",
    inputSchema: {
      query: z.string().describe("Search term to look for in title, contracting authority, or description"),
      limit: z.number().optional().default(10).describe("Maximum number of results to return (default: 10)")
    }
  },
  async ({ query, limit = 10 }) => {
    const client = await pool.connect();
    try {
      const result = await client.query(
        `SELECT resource_id, title, ca, published, deadline, status, value, pdf_url
         FROM tender_records 
         WHERE title ILIKE $1 OR ca ILIKE $1 OR info ILIKE $1
         ORDER BY published DESC 
         LIMIT $2`,
        [`%${query}%`, limit]
      );

      if (result.rows.length === 0) {
        return {
          content: [{
            type: "text",
            text: `No tenders found matching "${query}"`
          }]
        };
      }

      const tenders = result.rows.map(row => ({
        resource_id: row.resource_id,
        title: row.title,
        contracting_authority: row.ca,
        published: row.published,
        deadline: row.deadline,
        status: row.status,
        value: row.value,
        pdf_url: row.pdf_url
      }));

      return {
        content: [{
          type: "text",
          text: `Found ${result.rows.length} tenders matching "${query}":\n\n` +
                JSON.stringify(tenders, null, 2)
        }]
      };
    } finally {
      client.release();
    }
  }
);

// Get tender details tool
server.registerTool(
  "get_tender_details",
  {
    title: "Get Tender Details",
    description: "Get complete details for a specific tender by resource_id",
    inputSchema: {
      resource_id: z.string().describe("The resource_id of the tender to retrieve")
    }
  },
  async ({ resource_id }) => {
    const client = await pool.connect();
    try {
      const result = await client.query(
        'SELECT * FROM tender_records WHERE resource_id = $1',
        [resource_id]
      );

      if (result.rows.length === 0) {
        return {
          content: [{
            type: "text",
            text: `No tender found with resource_id: ${resource_id}`
          }]
        };
      }

      return {
        content: [{
          type: "text",
          text: JSON.stringify(result.rows[0], null, 2)
        }]
      };
    } finally {
      client.release();
    }
  }
);

// Filter tenders by criteria tool
server.registerTool(
  "filter_tenders",
  {
    title: "Filter Tenders",
    description: "Filter tenders by various criteria like status, contracting authority, date range, or value",
    inputSchema: {
      status: z.string().optional().describe("Filter by tender status (e.g., 'Open', 'Closed')"),
      contracting_authority: z.string().optional().describe("Filter by contracting authority name"),
      published_after: z.string().optional().describe("Filter tenders published after this date (YYYY-MM-DD)"),
      published_before: z.string().optional().describe("Filter tenders published before this date (YYYY-MM-DD)"),
      deadline_after: z.string().optional().describe("Filter tenders with deadline after this date (YYYY-MM-DD)"),
      deadline_before: z.string().optional().describe("Filter tenders with deadline before this date (YYYY-MM-DD)"),
      min_value: z.string().optional().describe("Minimum tender value (as string, will be compared as text)"),
      limit: z.number().optional().default(20).describe("Maximum number of results (default: 20)")
    }
  },
  async ({ status, contracting_authority, published_after, published_before, deadline_after, deadline_before, min_value, limit = 20 }) => {
    const client = await pool.connect();
    try {
      let query = 'SELECT resource_id, title, ca, published, deadline, status, value FROM tender_records WHERE 1=1';
      const params = [];
      let paramCount = 0;

      if (status) {
        paramCount++;
        query += ` AND status ILIKE $${paramCount}`;
        params.push(`%${status}%`);
      }

      if (contracting_authority) {
        paramCount++;
        query += ` AND ca ILIKE $${paramCount}`;
        params.push(`%${contracting_authority}%`);
      }

      if (published_after) {
        paramCount++;
        query += ` AND published >= $${paramCount}`;
        params.push(published_after);
      }

      if (published_before) {
        paramCount++;
        query += ` AND published <= $${paramCount}`;
        params.push(published_before);
      }

      if (deadline_after) {
        paramCount++;
        query += ` AND deadline >= $${paramCount}`;
        params.push(deadline_after);
      }

      if (deadline_before) {
        paramCount++;
        query += ` AND deadline <= $${paramCount}`;
        params.push(deadline_before);
      }

      if (min_value) {
        paramCount++;
        query += ` AND value IS NOT NULL AND value != '' AND CAST(REGEXP_REPLACE(value, '[^0-9.]', '', 'g') AS NUMERIC) >= $${paramCount}`;
        params.push(parseFloat(min_value));
      }

      query += ` ORDER BY published DESC LIMIT $${paramCount + 1}`;
      params.push(limit);

      const result = await client.query(query, params);

      return {
        content: [{
          type: "text",
          text: `Found ${result.rows.length} tenders matching the criteria:\n\n` +
                JSON.stringify(result.rows, null, 2)
        }]
      };
    } catch (error) {
      return {
        content: [{
          type: "text",
          text: `Error filtering tenders: ${error.message}`
        }],
        isError: true
      };
    } finally {
      client.release();
    }
  }
);

// Get tender statistics tool
server.registerTool(
  "get_tender_statistics",
  {
    title: "Get Tender Statistics",
    description: "Get summary statistics about the tender database",
    inputSchema: {}
  },
  async () => {
    const client = await pool.connect();
    try {
      const queries = [
        { name: 'total_tenders', query: 'SELECT COUNT(*) as count FROM tender_records' },
        { name: 'open_tenders', query: "SELECT COUNT(*) as count FROM tender_records WHERE status ILIKE '%open%'" },
        { name: 'closed_tenders', query: "SELECT COUNT(*) as count FROM tender_records WHERE status ILIKE '%closed%'" },
        { name: 'recent_tenders', query: "SELECT COUNT(*) as count FROM tender_records WHERE published >= CURRENT_DATE - INTERVAL '30 days'" },
        { name: 'top_authorities', query: 'SELECT ca, COUNT(*) as count FROM tender_records GROUP BY ca ORDER BY count DESC LIMIT 10' },
        { name: 'tender_statuses', query: 'SELECT status, COUNT(*) as count FROM tender_records GROUP BY status ORDER BY count DESC' }
      ];

      const results = {};
      
      for (const { name, query } of queries) {
        const result = await client.query(query);
        if (name === 'top_authorities' || name === 'tender_statuses') {
          results[name] = result.rows;
        } else {
          results[name] = result.rows[0].count;
        }
      }

      return {
        content: [{
          type: "text",
          text: `# Tender Database Statistics

**Total Tenders:** ${results.total_tenders}
**Open Tenders:** ${results.open_tenders}  
**Closed Tenders:** ${results.closed_tenders}
**Recent Tenders (last 30 days):** ${results.recent_tenders}

## Top Contracting Authorities
${results.top_authorities.map(row => `- ${row.ca}: ${row.count} tenders`).join('\n')}

## Tender Status Distribution  
${results.tender_statuses.map(row => `- ${row.status}: ${row.count} tenders`).join('\n')}`
        }]
      };
    } finally {
      client.release();
    }
  }
);

// Analyze tender opportunities tool
server.registerTool(
  "analyze_opportunities",
  {
    title: "Analyze Tender Opportunities",
    description: "Analyze current tender opportunities based on keywords or criteria relevant to your business",
    inputSchema: {
      keywords: z.array(z.string()).describe("Array of keywords relevant to your business (e.g., ['IT', 'software', 'consulting'])"),
      min_days_remaining: z.number().optional().default(7).describe("Minimum days remaining until deadline (default: 7)")
    }
  },
  async ({ keywords, min_days_remaining = 7 }) => {
    const client = await pool.connect();
    try {
      // Build keyword search conditions
      const keywordConditions = keywords.map((_, index) => 
        `(title ILIKE $${index + 1} OR info ILIKE $${index + 1})`
      ).join(' OR ');
      
      const keywordParams = keywords.map(keyword => `%${keyword}%`);

      const query = `
        SELECT resource_id, title, ca, published, deadline, status, value, pdf_url,
               CASE 
                 WHEN deadline ~ '^[0-9]{2}/[0-9]{2}/[0-9]{4}$' THEN 
                   (TO_DATE(deadline, 'DD/MM/YYYY') - CURRENT_DATE) 
                 ELSE NULL 
               END as days_remaining
        FROM tender_records 
        WHERE (${keywordConditions})
          AND status ILIKE '%open%'
          AND deadline IS NOT NULL 
          AND deadline != ''
          AND CASE 
            WHEN deadline ~ '^[0-9]{2}/[0-9]{2}/[0-9]{4}$' THEN 
              (TO_DATE(deadline, 'DD/MM/YYYY') - CURRENT_DATE) >= $${keywords.length + 1}
            ELSE FALSE 
          END
        ORDER BY days_remaining ASC`;

      const params = [...keywordParams, min_days_remaining];
      const result = await client.query(query, params);

      if (result.rows.length === 0) {
        return {
          content: [{
            type: "text",
            text: `No matching opportunities found for keywords: ${keywords.join(', ')}`
          }]
        };
      }

      const opportunities = result.rows.map(row => ({
        resource_id: row.resource_id,
        title: row.title,
        contracting_authority: row.ca,
        deadline: row.deadline,
        days_remaining: row.days_remaining,
        value: row.value,
        pdf_url: row.pdf_url
      }));

      return {
        content: [{
          type: "text",
          text: `# Tender Opportunities Analysis

Found ${opportunities.length} relevant opportunities for keywords: ${keywords.join(', ')}

## Opportunities (sorted by urgency):

${opportunities.map(opp => `
**${opp.title}**
- Resource ID: ${opp.resource_id}
- Authority: ${opp.contracting_authority}
- Deadline: ${opp.deadline} (${opp.days_remaining} days remaining)
- Value: ${opp.value}
- PDF: ${opp.pdf_url}
`).join('\n')}`
        }]
      };
    } catch (error) {
      return {
        content: [{
          type: "text",
          text: `Error analyzing opportunities: ${error.message}`
        }],
        isError: true
      };
    } finally {
      client.release();
    }
  }
);

// ==========================================
// SERVER STARTUP
// ==========================================

async function main() {
  console.log('🚀 Starting Irish Tenders MCP Server...');
  
  // Test database connection
  await testConnection();
  
  // Setup transport (stdio for Cursor integration)
  const transport = new StdioServerTransport();
  
  console.log('📡 Connecting to MCP transport...');
  await server.connect(transport);
  
  console.log('✅ Irish Tenders MCP Server is running!');
  console.log('📊 Available tools:');
  console.log('  - search_tenders: Search for tenders by keywords');
  console.log('  - get_tender_details: Get complete tender information');
  console.log('  - filter_tenders: Filter tenders by various criteria');
  console.log('  - get_tender_statistics: Get database statistics');
  console.log('  - analyze_opportunities: Analyze relevant business opportunities');
  console.log('');
  console.log('📁 Available resources:');
  console.log('  - tender://{resource_id}: Individual tender records');
  console.log('  - search://{query}: Search results');
}

// Handle graceful shutdown
process.on('SIGINT', async () => {
  console.log('\n🛑 Shutting down gracefully...');
  await pool.end();
  process.exit(0);
});

process.on('SIGTERM', async () => {
  console.log('\n🛑 Shutting down gracefully...');
  await pool.end();
  process.exit(0);
});

// Start the server
main().catch((error) => {
  console.error('❌ Failed to start server:', error);
  process.exit(1);
}); 