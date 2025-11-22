# MBRAS C2S API Documentation

> **Rust-based Lead Enrichment API** for Contact2Sale (C2S) integration with Work API and Diretrix.

**Version:** 1.0  
**Last Updated:** 2025-11-22  
**Repository:** https://github.com/MbInteligen/mbras-c2s-enrichment

---

## 🚀 Quick Start

- **[Getting Started Guide](QUICKSTART.md)** - Setup, run, and test the API
- **[API Reference](API_ENDPOINTS.md)** - Complete endpoint documentation

---

## 📚 Documentation Index

### 🏗️ Architecture

**[Architecture Decision Records (ADR)](adr/)**
- [ADR-001: Party Model Migration](adr/ADR-001-PARTY-MODEL-MIGRATION.md) - Migration from legacy entities to Party Model

**[Architecture Documentation](architecture/)**
- [Deduplication Implementation](architecture/DEDUPLICATION_IMPLEMENTATION.md) - Lead-level and CPF-level caching
- [Implementation Summary](architecture/IMPLEMENTATION_SUMMARY.md) - System architecture overview
- [Webhook + Redis Plan](architecture/PLAN_WEBHOOK_REDIS.md) - Direct C2S webhooks with Redis
- [On Close Lead Plan](architecture/PLAN_ON_CLOSE_LEAD.md) - Lead closure handling

---

### 🗄️ Database

**[Database Documentation](database/)**
- **[Database Schema Report](database/DATABASE_SCHEMA_REPORT_FINAL.md)** ⭐ - Complete production schema documentation
- [Address Confidence Scoring](database/ADDRESS_CONFIDENCE_SCORING.md) - Intelligent address confidence system
- [Schema Migration: Lead & Address](database/SCHEMA_MIGRATION_LEAD_ADDRESS.md) - C2S lead tracking migration
- [Database Hardening](database/DATABASE_HARDENING_COMPLETE.md) - Constraints, indexes, and validation
- [Database Analysis](database/DATABASE_ANALYSIS.md) - Schema analysis and comparisons
- [Data Comparison](database/DATA_COMPARISON.md) - Work API vs database data comparison
- [DB Storage Analysis](database/DB_STORAGE_ANALYSIS.md) - Storage layer architecture
- [DB Storage Analysis Updated](database/DB_STORAGE_ANALYSIS_UPDATED.md) - Updated storage patterns
- [Analytics Guide](database/ANALYTICS_GUIDE.md) - Materialized views and BI queries

**[Database Examples](database/examples/)**
- [Example CPF Response](database/examples/EXAMPLE_CPF_RESPONSE.json) - Sample Work API response
- [Wealth Assessment Example](database/examples/WEALTH_ASSESSMENT_EXAMPLE.json) - Sample wealth data

---

### 🚀 Deployment

**[Deployment Documentation](deployment/)**
- [Deployment Guide](deployment/DEPLOYMENT.md) - Complete deployment instructions
- [Deployment Checklist](deployment/DEPLOYMENT_CHECKLIST.md) - Pre-deployment verification
- [Deployment Status](deployment/DEPLOYMENT_STATUS.md) - Current deployment state
- [Google Ads Deployment Success](deployment/GOOGLE_ADS_DEPLOYMENT_SUCCESS.md) - Google Ads integration deployment
- [Validation Deployment](deployment/VALIDATION_DEPLOYMENT.md) - Post-deployment validation

---

### 🔌 Integrations

**[Integration Documentation](integrations/)**

**C2S Webhooks:**
- [C2S Webhook Configuration](integrations/C2S_WEBHOOK_CONFIGURATION.md) - Webhook setup guide
- [C2S Manual Webhook Setup](integrations/C2S_MANUAL_WEBHOOK_SETUP.md) - Manual webhook configuration
- [Webhook Implementation](integrations/WEBHOOK_IMPLEMENTATION.md) - Implementation details
- [Webhook Implementation Summary](integrations/WEBHOOK_IMPLEMENTATION_SUMMARY.md) - Summary overview
- [Webhook Deployment Steps](integrations/WEBHOOK_DEPLOYMENT_STEPS.md) - Step-by-step deployment
- [Webhook Subscription Status](integrations/WEBHOOK_SUBSCRIPTION_STATUS.md) - Current webhook status

**Google Ads:**
- [Google Ads Integration](integrations/GOOGLE_ADS_INTEGRATION.md) - Google Ads Lead Form integration
- [Google Ads Limitation](integrations/GOOGLE_ADS_LIMITATION.md) - Known limitations

**Other Integrations:**
- [Make.com Integration](integrations/MAKE_INTEGRATION.md) - Make.com workflow integration
- [Enrichment Integration](integrations/ENRICHMENT_INTEGRATION.md) - Enrichment flow documentation
- [Module Test Results](integrations/MODULE_TEST_RESULTS.md) - Work API module testing results

---

### 🔒 Security

**[Security Documentation](security/)**
- [Security Checklist](security/SECURITY_CHECKLIST.md) - Security best practices and verification
- [Security Fixes](security/SECURITY_FIXES.md) - Applied security patches
- [Security and Schema Fixes](security/SECURITY_AND_SCHEMA_FIXES.md) - Combined security/schema updates

---

### ✅ Testing

**[Testing Documentation](testing/)**
- [Testing Guide](testing/TESTING.md) - Testing strategies and procedures
- [Testing Complete](testing/TESTING_COMPLETE.md) - Test completion report
- [Performance Monitoring](testing/PERFORMANCE_MONITORING.md) - Performance metrics and monitoring

**Testing Scripts:** See [../tools/scripts/testing/](../tools/scripts/testing/)

---

### 📝 Session Notes

**[Development Session Summaries](session-notes/)**
- [Session Summary](session-notes/SESSION_SUMMARY.md) - Development session notes
- [Session Complete](session-notes/SESSION_COMPLETE.md) - Session completion summary
- [Final Status](session-notes/FINAL_STATUS.md) - Final project status
- [Implementation Complete](session-notes/IMPLEMENTATION_COMPLETE.md) - Implementation completion report
- [Project Summary](session-notes/PROJECT_SUMMARY.md) - Overall project summary
- [Deployment Complete](session-notes/DEPLOYMENT_COMPLETE.md) - Deployment completion notes
- [Deployment Ready](session-notes/DEPLOYMENT_READY.md) - Pre-deployment readiness
- [Implementation Summary](session-notes/IMPLEMENTATION_SUMMARY.md) - Implementation overview

---

## 🛠️ Development Resources

### Project Structure

```
rust-c2s-api/
├── docs/              # All documentation (you are here)
├── src/               # Rust source code
├── migrations/        # Database migrations
├── tools/             # Development tools & utilities
│   ├── scripts/      # Utility scripts
│   │   ├── deployment/   # Deployment scripts
│   │   ├── testing/      # Testing scripts
│   │   └── data/         # Data processing scripts
│   └── examples/     # Rust code examples
├── tests/             # Integration tests
└── temp_data/         # Temporary data files
```

### Key Files

- **[../README.md](../README.md)** - Main project README
- **[../CLAUDE.md](../CLAUDE.md)** - AI assistant context and project conventions
- **[../AGENTS.md](../AGENTS.md)** - Repository guidelines for contributors
- **[../Cargo.toml](../Cargo.toml)** - Rust dependencies and configuration
- **[../.env.example](../.env.example)** - Environment variables template

### Scripts

**Deployment:**
- [`../tools/scripts/deployment/RUN_SERVER.sh`](../tools/scripts/deployment/RUN_SERVER.sh) - Start local development server

**Testing:**
- [`../tools/scripts/testing/test-local.sh`](../tools/scripts/testing/test-local.sh) - Local environment tests
- [`../tools/scripts/testing/test_all_modules.sh`](../tools/scripts/testing/test_all_modules.sh) - Test all Work API modules
- [`../tools/scripts/testing/test_work_api.sh`](../tools/scripts/testing/test_work_api.sh) - Test Work API integration
- [`../tools/scripts/testing/test_webhook.sh`](../tools/scripts/testing/test_webhook.sh) - Test C2S webhooks
- [`../tools/scripts/testing/test_google_webhook.sh`](../tools/scripts/testing/test_google_webhook.sh) - Test Google Ads webhooks

**Data Processing:**
- [`../tools/scripts/data/enrich_batch.sh`](../tools/scripts/data/enrich_batch.sh) - Batch CPF enrichment
- [`../tools/scripts/data/import_enriched_to_db.sh`](../tools/scripts/data/import_enriched_to_db.sh) - Import enriched data to PostgreSQL
- [`../tools/scripts/data/retry_failed_cpfs.sh`](../tools/scripts/data/retry_failed_cpfs.sh) - Retry failed enrichments

---

## 🏷️ Tags & Categories

### By Topic

- **Architecture**: ADR, architecture/, database schema
- **Database**: database/, migrations/, DB analysis
- **Integration**: C2S webhooks, Google Ads, Make.com, Work API
- **Security**: security/, credentials management
- **Operations**: deployment/, testing/, monitoring
- **Development**: session-notes/, implementation summaries

### By Audience

- **Developers**: Architecture docs, API reference, testing guides
- **DevOps**: Deployment docs, security checklists
- **Business**: Project summaries, integration guides
- **Data Team**: Database schema, analytics guide

---

## 📞 Support

- **Repository Issues**: https://github.com/MbInteligen/mbras-c2s-enrichment/issues
- **Deployment**: Fly.io (https://mbras-c2s.fly.dev)
- **Database**: Neon.tech PostgreSQL (São Paulo region)

---

## 🔄 Recent Updates

**2025-11-22:**
- ✅ Party Model migration complete (1.5M+ records)
- ✅ Database schema documentation updated
- ✅ Documentation structure reorganized
- ✅ ADR-001 created for Party Model decision

**2025-11-21:**
- ✅ Database hardening complete
- ✅ Webhook events table created
- ✅ Google Ads lead integration deployed

---

**Documentation Version:** 1.0  
**Generated:** 2025-11-22  
**Maintained By:** MbInteligen Engineering Team