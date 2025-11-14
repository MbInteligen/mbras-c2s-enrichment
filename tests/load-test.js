// k6 Load Test for rust-c2s-api
// Install k6: https://k6.io/docs/getting-started/installation/
// Run: k6 run tests/load-test.js
// Run with custom URL: k6 run -e BASE_URL=https://your-app.fly.dev tests/load-test.js

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate } from 'k6/metrics';

// Custom metrics
const errorRate = new Rate('errors');

// Test configuration
export const options = {
  stages: [
    { duration: '30s', target: 5 },   // Ramp up to 5 users
    { duration: '1m', target: 10 },   // Stay at 10 users
    { duration: '30s', target: 20 },  // Peak at 20 users
    { duration: '1m', target: 20 },   // Hold peak
    { duration: '30s', target: 0 },   // Ramp down
  ],
  thresholds: {
    'http_req_duration': ['p(95)<2000'], // 95% of requests should complete in <2s
    'http_req_failed': ['rate<0.1'],      // Error rate should be <10%
    'errors': ['rate<0.1'],
  },
};

const BASE_URL = __ENV.BASE_URL || 'http://localhost:8081';
const LEAD_ID = __ENV.LEAD_ID || '358f62821dc6cfa7cfbda19e670d6392';

export default function () {
  // Test 1: Health Check (lightweight)
  let healthRes = http.get(`${BASE_URL}/health`);
  check(healthRes, {
    'health check status is 200': (r) => r.status === 200,
  }) || errorRate.add(1);

  sleep(1);

  // Test 2: Get Customer by CPF (database query)
  let customerRes = http.get(`${BASE_URL}/api/v1/contributor/customer?cpf=12345678900`);
  check(customerRes, {
    'customer query completed': (r) => r.status >= 200 && r.status < 500,
  }) || errorRate.add(1);

  sleep(2);

  // Test 3: Trigger Lead Processing (full enrichment flow)
  // WARNING: This hits external APIs (C2S, Diretrix, Work API)
  // Use sparingly to avoid rate limits
  if (__ITER % 10 === 0) { // Only 1 in 10 iterations
    let triggerRes = http.get(`${BASE_URL}/api/v1/leads/process?id=${LEAD_ID}`);
    check(triggerRes, {
      'trigger processing responded': (r) => r.status >= 200 && r.status < 500,
      'trigger processing success': (r) => {
        try {
          const body = JSON.parse(r.body);
          return body.success === true;
        } catch {
          return false;
        }
      },
    }) || errorRate.add(1);

    sleep(5); // Longer sleep after heavy operation
  }

  sleep(1);
}

export function handleSummary(data) {
  return {
    'stdout': textSummary(data, { indent: ' ', enableColors: true }),
    'load-test-results.json': JSON.stringify(data),
  };
}

function textSummary(data, options) {
  const indent = options.indent || '';
  const colors = options.enableColors;

  let summary = '\n' + indent + '📊 Load Test Summary\n';
  summary += indent + '━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n';

  const metrics = data.metrics;

  // HTTP metrics
  if (metrics.http_reqs) {
    summary += indent + `Total Requests: ${metrics.http_reqs.values.count}\n`;
  }

  if (metrics.http_req_duration) {
    summary += indent + `Request Duration:\n`;
    summary += indent + `  Average: ${metrics.http_req_duration.values.avg.toFixed(2)}ms\n`;
    summary += indent + `  Median:  ${metrics.http_req_duration.values.med.toFixed(2)}ms\n`;
    summary += indent + `  P95:     ${metrics.http_req_duration.values['p(95)'].toFixed(2)}ms\n`;
    summary += indent + `  Max:     ${metrics.http_req_duration.values.max.toFixed(2)}ms\n`;
  }

  if (metrics.http_req_failed) {
    const failRate = (metrics.http_req_failed.values.rate * 100).toFixed(2);
    summary += indent + `Failed Requests: ${failRate}%\n`;
  }

  if (metrics.errors) {
    const errorRatePct = (metrics.errors.values.rate * 100).toFixed(2);
    summary += indent + `Error Rate: ${errorRatePct}%\n`;
  }

  summary += indent + '\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n';
  summary += indent + '💡 Next steps:\n';
  summary += indent + '  - Check fly logs for errors\n';
  summary += indent + '  - Monitor memory: fly status --app rust-c2s-api\n';
  summary += indent + '  - Adjust VM size if needed in fly.toml\n';

  return summary;
}
