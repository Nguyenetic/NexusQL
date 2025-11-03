# Fix GitHub Packages Publish Error (403 Permission Denied)

## 🔴 **The Problem**

Error: `403 Forbidden - permission_denied: create_package`

**Cause**: Your GitHub token doesn't have the right permissions.

---

## ✅ **Solution: Create New Token with Correct Permissions**

### **Step 1: Go to GitHub Token Settings**

https://github.com/settings/tokens

### **Step 2: Delete Old Token**

Find your old token and delete it (it doesn't have enough permissions).

### **Step 3: Generate New Token (Classic)**

Click **"Generate new token (classic)"**

**Token name**: NexusQL Package Publishing

**Select these scopes (IMPORTANT!):**
- ✅ `repo` (Full control of private repositories)
- ✅ `write:packages` (Upload packages to GitHub Package Registry)
- ✅ `read:packages` (Download packages from GitHub Package Registry)
- ✅ `delete:packages` (Delete packages from GitHub Package Registry)

Click **"Generate token"**

**Copy the new token!**

---

## 🔧 **Step 4: Update .npmrc with New Token**

Edit `C:\Users\jnguyen\.npmrc`:

```
@nhoy:registry=https://npm.pkg.github.com
//npm.pkg.github.com/:_authToken=YOUR_NEW_TOKEN_HERE
```

**Or in PowerShell:**
```powershell
@"
@nhoy:registry=https://npm.pkg.github.com
//npm.pkg.github.com/:_authToken=YOUR_NEW_TOKEN_HERE
"@ | Out-File -FilePath "$env:USERPROFILE\.npmrc" -Encoding ASCII -Force
```

---

## 🚀 **Step 5: Try Publishing Again**

```powershell
cd C:\Users\jnguyen\Documents\Project\NexusQL\bindings\typescript
npm publish
```

**Should work now!** ✅

---

## 🎯 **Checklist**

Make sure your token has:
- ✅ `repo` scope (for private repos)
- ✅ `write:packages` scope (to publish)
- ✅ Token is NOT expired
- ✅ .npmrc file is in the right location (`C:\Users\jnguyen\.npmrc`)

---

## 📋 **Alternative: Authenticate with npm login**

If tokens don't work, try:

```bash
npm login --registry=https://npm.pkg.github.com
Username: nhoy
Password: YOUR_GITHUB_TOKEN_HERE
Email: your-email@example.com
```

---

## ✅ **After Publishing Successfully:**

Your package will be at:
- **GitHub**: https://github.com/johnhnguyen97/NexusQL/packages

**Install anywhere:**
```bash
npm install @nhoy/nexusql
```

---

**The issue is just token permissions - create a new token with `repo` and `write:packages` scopes!** 🔑
