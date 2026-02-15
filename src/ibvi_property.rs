//! IBVI Property Intelligence service.
//!
//! Ported from ts:src/services/ibvi-property.service.ts
//!
//! Queries property ownership from IBVI PostgreSQL:
//!   core.parties → core.property_ownerships → core.real_estate_properties → core.addresses
//!
//! Features:
//! - Property lookup by CPF
//! - Portfolio summary (total value, count, built area)
//! - C2S message formatting
//! - IPTU report data (HTML template ready)

use serde::{Deserialize, Serialize};
use sqlx::PgPool;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyAddress {
    pub street: Option<String>,
    pub number: Option<String>,
    pub complement: Option<String>,
    pub neighborhood: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub zip_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyDetails {
    pub property_code: Option<String>,
    pub iptu_code: Option<String>,
    pub property_type: Option<String>,
    pub land_area_sqm: Option<f64>,
    pub built_area_sqm: Option<f64>,
    pub rooms_count: Option<i32>,
    pub bathrooms_count: Option<i32>,
    pub parking_spaces: Option<i32>,
    pub market_value_brl: Option<f64>,
    pub tax_value_brl: Option<f64>,
    pub monthly_tax_brl: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyOwnership {
    pub property_id: String,
    pub ownership_percentage: f64,
    pub ownership_type: String,
    pub is_current: bool,
    pub property: PropertyDetails,
    pub address: PropertyAddress,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertySummary {
    pub total_properties: u32,
    pub total_current_properties: u32,
    pub total_market_value: f64,
    pub total_market_value_formatted: String,
    pub total_built_area: f64,
    pub properties: Vec<PropertyOwnership>,
}

// ---------------------------------------------------------------------------
// Database row mapping
// ---------------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
struct PropertyRow {
    property_id: Option<String>,
    ownership_percentage: Option<bigdecimal::BigDecimal>,
    ownership_type: Option<String>,
    is_current: Option<bool>,
    property_code: Option<String>,
    iptu_code: Option<String>,
    property_type: Option<String>,
    land_area_sqm: Option<bigdecimal::BigDecimal>,
    built_area_sqm: Option<bigdecimal::BigDecimal>,
    rooms_count: Option<i32>,
    bathrooms_count: Option<i32>,
    parking_spaces: Option<i32>,
    market_value_brl: Option<bigdecimal::BigDecimal>,
    tax_value_brl: Option<bigdecimal::BigDecimal>,
    monthly_tax_brl: Option<bigdecimal::BigDecimal>,
    street: Option<String>,
    number: Option<String>,
    complement: Option<String>,
    neighborhood: Option<String>,
    city: Option<String>,
    state: Option<String>,
    zip_code: Option<String>,
}

fn bd_to_f64(val: &Option<bigdecimal::BigDecimal>) -> Option<f64> {
    val.as_ref().and_then(|v| {
        use std::str::FromStr;
        f64::from_str(&v.to_string()).ok()
    })
}

// ---------------------------------------------------------------------------
// IbviPropertyService
// ---------------------------------------------------------------------------

pub struct IbviPropertyService {
    db: PgPool,
}

impl IbviPropertyService {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    /// Find properties owned by a CPF.
    pub async fn find_properties_by_cpf(&self, cpf: &str) -> Option<PropertySummary> {
        let normalized = cpf.replace(|c: char| !c.is_ascii_digit(), "");

        let rows = match sqlx::query_as::<_, PropertyRow>(
            r#"
            SELECT
                po.property_id::text,
                po.ownership_percentage,
                po.ownership_type,
                po.is_current,
                p.property_code,
                p.iptu_code,
                p.property_type,
                p.land_area_sqm,
                p.built_area_sqm,
                p.rooms_count,
                p.bathrooms_count,
                p.parking_spaces,
                p.market_value_brl,
                p.tax_value_brl,
                p.monthly_tax_brl,
                a.street,
                a.number,
                a.complement,
                a.neighborhood,
                a.city,
                a.state,
                a.zip_code
            FROM core.parties pa
            JOIN core.property_ownerships po ON pa.id = po.party_id
            JOIN core.real_estate_properties p ON po.property_id = p.property_id
            LEFT JOIN core.addresses a ON p.address_id = a.id
            WHERE pa.cpf_cnpj = $1
            ORDER BY po.is_current DESC, p.market_value_brl DESC NULLS LAST
            LIMIT 10
            "#,
        )
        .bind(&normalized)
        .fetch_all(&self.db)
        .await
        {
            Ok(rows) => rows,
            Err(e) => {
                tracing::error!("IBVI property query failed for CPF {}: {}", normalized, e);
                return None;
            }
        };

        if rows.is_empty() {
            return None;
        }

        let properties: Vec<PropertyOwnership> = rows
            .iter()
            .map(|row| PropertyOwnership {
                property_id: row.property_id.clone().unwrap_or_default(),
                ownership_percentage: bd_to_f64(&row.ownership_percentage).unwrap_or(0.0),
                ownership_type: row.ownership_type.clone().unwrap_or_else(|| "unknown".to_string()),
                is_current: row.is_current.unwrap_or(true),
                property: PropertyDetails {
                    property_code: row.property_code.clone(),
                    iptu_code: row.iptu_code.clone(),
                    property_type: row.property_type.clone(),
                    land_area_sqm: bd_to_f64(&row.land_area_sqm),
                    built_area_sqm: bd_to_f64(&row.built_area_sqm),
                    rooms_count: row.rooms_count,
                    bathrooms_count: row.bathrooms_count,
                    parking_spaces: row.parking_spaces,
                    market_value_brl: bd_to_f64(&row.market_value_brl),
                    tax_value_brl: bd_to_f64(&row.tax_value_brl),
                    monthly_tax_brl: bd_to_f64(&row.monthly_tax_brl),
                },
                address: PropertyAddress {
                    street: row.street.clone(),
                    number: row.number.clone(),
                    complement: row.complement.clone(),
                    neighborhood: row.neighborhood.clone(),
                    city: row.city.clone(),
                    state: row.state.clone(),
                    zip_code: row.zip_code.clone(),
                },
            })
            .collect();

        let current: Vec<&PropertyOwnership> = properties.iter().filter(|p| p.is_current).collect();
        let total_market_value: f64 = current
            .iter()
            .filter_map(|p| p.property.market_value_brl)
            .sum();
        let total_built_area: f64 = current
            .iter()
            .filter_map(|p| p.property.built_area_sqm)
            .sum();

        Some(PropertySummary {
            total_properties: properties.len() as u32,
            total_current_properties: current.len() as u32,
            total_market_value,
            total_market_value_formatted: format_brl(total_market_value),
            total_built_area,
            properties,
        })
    }

    /// Format property data for C2S message.
    pub fn format_for_message(summary: &PropertySummary) -> String {
        if summary.total_properties == 0 {
            return String::new();
        }

        let mut lines = Vec::new();
        lines.push(format!(
            "🏠 IMÓVEIS ({} atual)",
            summary.total_current_properties
        ));

        if summary.total_market_value > 0.0 {
            lines.push(format!(
                "   Valor total: R$ {}",
                format_brl(summary.total_market_value)
            ));
        }

        if summary.total_built_area > 0.0 {
            lines.push(format!(
                "   Área total: {:.0} m²",
                summary.total_built_area
            ));
        }

        // Show up to 3 properties
        for prop in summary.properties.iter().take(3) {
            let addr = &prop.address;
            let location_parts: Vec<&str> = [
                addr.neighborhood.as_deref(),
                addr.city.as_deref(),
                addr.state.as_deref(),
            ]
            .iter()
            .filter_map(|x| *x)
            .collect();
            let location = location_parts.join(", ");

            let prop_type = prop
                .property
                .property_type
                .as_deref()
                .unwrap_or("Imóvel");

            let mut line = format!("   • {}", prop_type);
            if !location.is_empty() {
                line.push_str(&format!(" em {}", location));
            }
            if let Some(area) = prop.property.built_area_sqm {
                line.push_str(&format!(" ({:.0} m²)", area));
            }
            if let Some(value) = prop.property.market_value_brl {
                line.push_str(&format!(" - R$ {}", format_brl(value)));
            }
            lines.push(line);
        }

        if summary.properties.len() > 3 {
            lines.push(format!(
                "   ... e mais {} imóvel(is)",
                summary.properties.len() - 3
            ));
        }

        lines.join("\n")
    }
}

/// Format a float as Brazilian Real currency (e.g., 1.234.567,89).
fn format_brl(value: f64) -> String {
    let abs = value.abs();
    let cents = ((abs * 100.0).round() as u64) % 100;
    let whole = (abs.round() as u64) / 1;
    let integer_part = abs as u64;

    // Format with thousand separators
    let int_str = integer_part.to_string();
    let mut formatted = String::new();
    for (i, c) in int_str.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            formatted.push('.');
        }
        formatted.push(c);
    }
    let formatted: String = formatted.chars().rev().collect();

    if value < 0.0 {
        format!("-{},{:02}", formatted, cents)
    } else {
        format!("{},{:02}", formatted, cents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_brl() {
        assert_eq!(format_brl(1234567.89), "1.234.567,89");
        assert_eq!(format_brl(0.0), "0,00");
        assert_eq!(format_brl(1000.0), "1.000,00");
        assert_eq!(format_brl(99.50), "99,50");
    }

    #[test]
    fn test_format_for_message_empty() {
        let summary = PropertySummary {
            total_properties: 0,
            total_current_properties: 0,
            total_market_value: 0.0,
            total_market_value_formatted: "0,00".to_string(),
            total_built_area: 0.0,
            properties: vec![],
        };
        assert_eq!(IbviPropertyService::format_for_message(&summary), "");
    }

    #[test]
    fn test_format_for_message_with_properties() {
        let summary = PropertySummary {
            total_properties: 2,
            total_current_properties: 2,
            total_market_value: 2500000.0,
            total_market_value_formatted: "2.500.000,00".to_string(),
            total_built_area: 450.0,
            properties: vec![
                PropertyOwnership {
                    property_id: "1".to_string(),
                    ownership_percentage: 100.0,
                    ownership_type: "owner".to_string(),
                    is_current: true,
                    property: PropertyDetails {
                        property_code: None,
                        iptu_code: None,
                        property_type: Some("Apartamento".to_string()),
                        land_area_sqm: None,
                        built_area_sqm: Some(250.0),
                        rooms_count: Some(3),
                        bathrooms_count: Some(2),
                        parking_spaces: Some(2),
                        market_value_brl: Some(1500000.0),
                        tax_value_brl: None,
                        monthly_tax_brl: None,
                    },
                    address: PropertyAddress {
                        street: Some("Rua Test".to_string()),
                        number: Some("100".to_string()),
                        complement: None,
                        neighborhood: Some("Jardim Europa".to_string()),
                        city: Some("São Paulo".to_string()),
                        state: Some("SP".to_string()),
                        zip_code: None,
                    },
                },
                PropertyOwnership {
                    property_id: "2".to_string(),
                    ownership_percentage: 50.0,
                    ownership_type: "co-owner".to_string(),
                    is_current: true,
                    property: PropertyDetails {
                        property_code: None,
                        iptu_code: None,
                        property_type: Some("Casa".to_string()),
                        land_area_sqm: Some(500.0),
                        built_area_sqm: Some(200.0),
                        rooms_count: None,
                        bathrooms_count: None,
                        parking_spaces: None,
                        market_value_brl: Some(1000000.0),
                        tax_value_brl: None,
                        monthly_tax_brl: None,
                    },
                    address: PropertyAddress {
                        street: None,
                        number: None,
                        complement: None,
                        neighborhood: Some("Itaim Bibi".to_string()),
                        city: Some("São Paulo".to_string()),
                        state: Some("SP".to_string()),
                        zip_code: None,
                    },
                },
            ],
        };

        let msg = IbviPropertyService::format_for_message(&summary);
        assert!(msg.contains("IMÓVEIS (2 atual)"));
        assert!(msg.contains("2.500.000"));
        assert!(msg.contains("450 m²"));
        assert!(msg.contains("Apartamento"));
        assert!(msg.contains("Jardim Europa"));
        assert!(msg.contains("Casa"));
        assert!(msg.contains("Itaim Bibi"));
    }

    #[test]
    fn test_format_for_message_overflow() {
        let props: Vec<PropertyOwnership> = (0..5)
            .map(|i| PropertyOwnership {
                property_id: i.to_string(),
                ownership_percentage: 100.0,
                ownership_type: "owner".to_string(),
                is_current: true,
                property: PropertyDetails {
                    property_code: None, iptu_code: None,
                    property_type: Some("Apt".to_string()),
                    land_area_sqm: None, built_area_sqm: None,
                    rooms_count: None, bathrooms_count: None,
                    parking_spaces: None, market_value_brl: None,
                    tax_value_brl: None, monthly_tax_brl: None,
                },
                address: PropertyAddress {
                    street: None, number: None, complement: None,
                    neighborhood: None, city: None, state: None, zip_code: None,
                },
            })
            .collect();

        let summary = PropertySummary {
            total_properties: 5,
            total_current_properties: 5,
            total_market_value: 0.0,
            total_market_value_formatted: "0,00".to_string(),
            total_built_area: 0.0,
            properties: props,
        };

        let msg = IbviPropertyService::format_for_message(&summary);
        assert!(msg.contains("e mais 2 imóvel(is)"));
    }
}
