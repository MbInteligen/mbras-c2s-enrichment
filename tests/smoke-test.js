// k6 Smoke Test for rust-c2s-api
// Quick validation with minimal load
// Run: k6 run tests/smoke-test.js

import http from 'k6/http';
import { check, group } from 'k6';

export const options = {
  vus: 1, // 1 virtual user
  duration: '30s',
  thresholds: {
    'http_req_duration': ['p(95)<3000'], // 95% under 3s
    'http_req_failed': ['rate<0.05'],     // <5% errors
  },
};

const BASE_URL = __ENV.BASE_URL || 'http://localhost:8081';

export default function () {
  group('Basic Endpoints', function () {
    // Health check
    const health = http.get(`${BASE_URL}/health`);
    check(health, {
      'health is 200': (r) => r.status === 200,
    });

    // Get customer (should handle not found gracefully)
    const customer = http.get(`${BASE_URL}/api/v1/contributor/customer?cpf=00000000000`);
    check(customer, {
      'customer endpoint responds': (r) => r.status >= 200 && r.status < 500,
    });
  });
}
