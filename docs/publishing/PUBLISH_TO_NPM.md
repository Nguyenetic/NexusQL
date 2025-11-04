# How to Publish NexusQL to NPM (Private)

Simple guide based on your NPM account.

---

## ✅ **IMPORTANT: Remove "private": true**

**Do NOT include `"private": true` in package.json!**

From the NPM docs:
- `"private": true` = NPM **refuses to publish** (for local-only packages)
- For private packages on NPM, use `--access restricted` instead

---

## 📦 **Publishing Steps**

### 1. Update package.json (Already Done!)

Your `bindings/typescript/package.json` now has:
```json
{
  "name": "@johnhnguyen97/nexusql",
  "version": "2.4.0",
  // NO "private": true here!
}
```

### 2. Login to NPM

```bash
npm login
```
Enter your credentials.

### 3. Publish (Private)

```bash
cd C:/Users/jnguyen/Documents/Project/NexusQL/bindings/typescript

# For scoped packages (@username/package), default is already RESTRICTED
npm publish

# Or be explicit:
npm publish --access restricted
```

**That's it!** Your package is now on NPM as **PRIVATE**.

---

## 🎯 **Installing Your Private Package**

### In ANY Node.js Project:

```bash
npm install @johnhnguyen97/nexusql
```

Works exactly like public NPM, but only you (and people you grant access to) can install it!

---

## 👥 **Sharing with Team Members**

If you want others to use your private package:

```bash
# Grant access to a user
npm owner add theirusername @johnhnguyen97/nexusql

# Or create a team (requires organization - $7/month)
npm org create yourorg
npm team create yourorg:developers
npm team add yourorg:developers theirusername
npm access grant read-write yourorg:developers @johnhnguyen97/nexusql
```

---

## 🔄 **Updating Your Package**

```bash
cd bindings/typescript

# 1. Update version in package.json (2.4.0 → 2.4.1)
# 2. Publish new version
npm publish
```

Users update with:
```bash
npm update @johnhnguyen97/nexusql
```

---

## 🎊 **Summary**

### **What You Need to Do:**

```bash
cd C:/Users/jnguyen/Documents/Project/NexusQL/bindings/typescript

# 1. Login (one time)
npm login

# 2. Publish (package.json is already configured!)
npm publish
```

### **Then Use Anywhere:**

```bash
npm install @johnhnguyen97/nexusql
```

**Key Points:**
- ✅ NO `"private": true` in package.json (already removed)
- ✅ Scoped package `@johnhnguyen97/nexusql` (already set)
- ✅ Default access is RESTRICTED for scoped packages
- ✅ FREE for your personal account
- ✅ Works exactly like public NPM

**Ready to publish!** 🚀
