import { Stack } from 'expo-router';
import { useCallback, useEffect, useState } from 'react';
import { ActivityIndicator, Pressable, StyleSheet, Text, View } from 'react-native';

import { api, type Health } from '@/api/client';
import { resolveApiBaseUrl } from '@/api/baseUrl';
import { colors } from '@/theme';

type State =
  | { kind: 'loading' }
  | { kind: 'ok'; health: Health }
  | { kind: 'error'; message: string };

export default function Home() {
  const [state, setState] = useState<State>({ kind: 'loading' });

  const check = useCallback(async () => {
    setState({ kind: 'loading' });
    try {
      setState({ kind: 'ok', health: await api.health() });
    } catch (e) {
      setState({ kind: 'error', message: e instanceof Error ? e.message : String(e) });
    }
  }, []);

  useEffect(() => {
    check();
  }, [check]);

  return (
    <View style={styles.container}>
      <Stack.Screen options={{ title: 'Christmas Bingo' }} />
      <Text style={styles.hero}>🎄🎅⛄</Text>
      <Text style={styles.title}>Christmas Bingo</Text>

      <View style={styles.card}>
        {state.kind === 'loading' && <ActivityIndicator color={colors.green} />}
        {state.kind === 'ok' && (
          <Text style={[styles.status, { color: colors.green }]}>
            Server is ready ✓ (v{state.health.version})
          </Text>
        )}
        {state.kind === 'error' && (
          <>
            <Text style={[styles.status, { color: colors.red }]}>Can't reach the server</Text>
            <Text style={styles.detail}>{state.message}</Text>
          </>
        )}
        <Text style={styles.detail}>{resolveApiBaseUrl()}</Text>
      </View>

      <Pressable style={styles.button} onPress={check} accessibilityRole="button">
        <Text style={styles.buttonText}>Check again</Text>
      </Pressable>
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, alignItems: 'center', justifyContent: 'center', padding: 16, gap: 16 },
  hero: { fontSize: 56 },
  title: { fontSize: 32, fontWeight: '800', color: colors.green },
  card: {
    width: '100%',
    maxWidth: 420,
    alignItems: 'center',
    gap: 8,
    padding: 20,
    borderRadius: 16,
    backgroundColor: '#fff',
    borderWidth: 2,
    borderColor: colors.gold,
  },
  status: { fontSize: 18, fontWeight: '700' },
  detail: { fontSize: 13, color: colors.muted, textAlign: 'center' },
  button: {
    minHeight: 56,
    paddingHorizontal: 28,
    justifyContent: 'center',
    borderRadius: 28,
    backgroundColor: colors.red,
  },
  buttonText: { color: colors.snow, fontSize: 18, fontWeight: '700' },
});
