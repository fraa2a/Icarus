import { invoke } from '@tauri-apps/api/core'

export interface MigrationInstance {
	id: string
	name: string
	game_version: string
	loader: string
}

export function migrationInstances() {
	return invoke<MigrationInstance[]>('plugin:icarus|migration_instances')
}

export function importModrinth(instanceIds: string[], removeSource: boolean) {
	return invoke<string[]>('plugin:icarus|import_modrinth', { instanceIds, removeSource })
}

export function runCopy(instanceId: string, accountId: string) {
	return invoke<{ instance_id: string }>('plugin:icarus|run_copy', { instanceId, accountId })
}
