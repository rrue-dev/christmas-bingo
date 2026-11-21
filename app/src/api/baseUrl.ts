import { Platform } from 'react-native';

/**
 * Where the bingo server lives.
 *
 * Precedence:
 * 1. `EXPO_PUBLIC_API_BASE_URL` (native/Expo Go, or web served by Metro during dev)
 * 2. On web: the page's own origin. In production the Rust server serves this
 *    bundle, so API and app share one origin and one QR URL.
 * 3. Fallback for local dev.
 */
export function resolveApiBaseUrl(
  envValue: string | undefined = process.env.EXPO_PUBLIC_API_BASE_URL,
  platform: string = Platform.OS,
  origin: string | undefined = typeof window !== 'undefined' ? window.location?.origin : undefined,
): string {
  const fromEnv = envValue?.trim();
  if (fromEnv) return stripTrailingSlash(fromEnv);
  if (platform === 'web' && origin) return stripTrailingSlash(origin);
  return 'http://localhost:8080';
}

function stripTrailingSlash(url: string): string {
  return url.replace(/\/+$/, '');
}
