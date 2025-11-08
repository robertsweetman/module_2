# Manual Cleanup Steps for Security Group Migration

## Problem
Terraform is trying to modify security groups that are attached to RDS, which fails because it can't detach the RDS ENI. We need to manually remove the cross-referencing rules from AWS first.

## What You Need to Delete

You need to remove **ONLY** the rules that reference other security groups. Keep all rules that use CIDR blocks (like 0.0.0.0/0).

## Step-by-Step Instructions

### Step 1: Go to AWS Console

1. Log into AWS Console
2. Go to **EC2** service
3. In the left sidebar, click **Security Groups** (under "Network & Security")

---

### Step 2: Clean Up Lambda Security Group

1. Find the security group named **`lambda-sg`**
2. Click on it
3. Go to the **Outbound rules** tab
4. Find and DELETE this rule:
   - **Type**: Custom TCP
   - **Port**: 5432
   - **Destination**: (references `postgres-sg` security group)
   - Click **Edit outbound rules** → Find the rule → Click **Delete** → **Save rules**

5. **KEEP these rules** (don't delete):
   - HTTPS (443) to 0.0.0.0/0
   - HTTP (80) to 0.0.0.0/0

---

### Step 3: Clean Up Bastion Security Group

1. Find the security group named **`bastion-sg`**
2. Click on it
3. Go to the **Outbound rules** tab
4. Find and DELETE this rule:
   - **Type**: Custom TCP
   - **Port**: 5432
   - **Destination**: (references `postgres-sg` security group)
   - Click **Edit outbound rules** → Find the rule → Click **Delete** → **Save rules**

5. **KEEP this rule** (don't delete):
   - HTTPS (443) to 0.0.0.0/0

---

### Step 4: Clean Up Postgres Security Group

1. Find the security group named **`postgres-sg`**
2. Click on it
3. Go to the **Inbound rules** tab
4. DELETE ALL rules that reference other security groups:
   - **Type**: PostgreSQL (or Custom TCP)
   - **Port**: 5432
   - **Source**: (references `lambda-sg`)
   - Click **Edit inbound rules** → Find the rule → Click **Delete**
   
   - **Type**: PostgreSQL (or Custom TCP)
   - **Port**: 5432
   - **Source**: (references `bastion-sg`)
   - Find this rule → Click **Delete** → **Save rules**

5. The postgres-sg should now have **NO inbound rules** and **NO outbound rules**

---

## Summary of What to Delete

| Security Group | Rule Type | Protocol | Port | Source/Destination | Action |
|---------------|-----------|----------|------|-------------------|--------|
| lambda-sg | Egress | TCP | 5432 | postgres-sg | DELETE |
| bastion-sg | Egress | TCP | 5432 | postgres-sg | DELETE |
| postgres-sg | Ingress | TCP | 5432 | lambda-sg | DELETE |
| postgres-sg | Ingress | TCP | 5432 | bastion-sg | DELETE |

## What NOT to Delete

- **DO NOT delete** any rules with CIDR blocks (0.0.0.0/0, etc.)
- **DO NOT delete** the security groups themselves
- **DO NOT delete** HTTP/HTTPS rules to 0.0.0.0/0

---

## After Manual Cleanup

Once you've deleted those 4 rules in AWS Console:

1. The security groups will still exist (attached to RDS, Lambda, Bastion)
2. But they won't have the cross-referencing rules anymore
3. Terraform will be able to work without trying to detach ENIs

### Then Run the Migration Workflow Again

Go back to GitHub Actions and run the migration workflow again. This time it should:
1. Re-import the security groups (now without the problematic rules)
2. Create the new separate `aws_security_group_rule` resources
3. Successfully apply without errors

---

## Verification

After running the migration workflow, verify:

```bash
# Check that Lambda can still connect to RDS
# Check that Bastion can still connect to RDS
# The connectivity should be restored by the new aws_security_group_rule resources
```

---

## Why This Works

- **Before**: Security groups had inline rules referencing each other (circular dependency)
- **Manual step**: Remove the inline rules from AWS (breaks the cycle temporarily)
- **After Terraform**: Terraform creates separate rule resources (no more circular dependency)
- **Result**: Same connectivity, but cleaner infrastructure-as-code structure