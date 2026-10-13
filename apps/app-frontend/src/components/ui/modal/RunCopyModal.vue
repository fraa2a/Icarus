<script setup lang="ts">
import { PlayIcon, SpinnerIcon } from '@modrinth/assets'
import { Button, commonMessages, defineMessages, NewModal, useVIntl } from '@modrinth/ui'
import { useQueryClient } from '@tanstack/vue-query'
import { ref } from 'vue'
import { useRouter } from 'vue-router'

import { get_default_user, users } from '@/helpers/auth.js'
import { toError } from '@/helpers/errors'
import { runCopy } from '@/helpers/icarus'
import type { GameInstance } from '@/helpers/types'
import { instanceKeys } from '@/pages/instance/query-options'

const props = defineProps<{ instance: GameInstance; playing: boolean }>()
const { formatMessage } = useVIntl()
const messages = defineMessages({
	title: { id: 'icarus.run-copy.title', defaultMessage: 'Launch a copy with another account' },
	body: {
		id: 'icarus.run-copy.body',
		defaultMessage:
			'Create an independent copy of this instance and launch it with the selected Minecraft account. Each copy has its own console.',
	},
	saves: {
		id: 'icarus.run-copy.saves',
		defaultMessage:
			'While the original instance is running, its saves are excluded from the copy to protect your worlds.',
	},
	account: { id: 'icarus.run-copy.account', defaultMessage: 'Minecraft account' },
	empty: {
		id: 'icarus.run-copy.empty',
		defaultMessage: 'Sign in to a Minecraft account in Settings, then try again.',
	},
	launch: { id: 'icarus.run-copy.launch', defaultMessage: 'Create and launch copy' },
	busy: { id: 'icarus.run-copy.busy', defaultMessage: 'Creating and launching…' },
})
interface MinecraftAccount {
	profile: { id: string; name: string }
}
const modal = ref<InstanceType<typeof NewModal>>()
const accounts = ref<MinecraftAccount[]>([])
const accountId = ref('')
const loading = ref(false)
const busy = ref(false)
const error = ref('')
const router = useRouter()
const queryClient = useQueryClient()

async function show() {
	if (
		busy.value ||
		loading.value ||
		props.instance.quarantined ||
		props.instance.install_stage !== 'installed'
	)
		return
	error.value = ''
	accounts.value = []
	accountId.value = ''
	modal.value?.show()
	loading.value = true
	try {
		const [accountList, defaultAccount] = await Promise.all([users(), get_default_user()])
		accounts.value = Array.isArray(accountList) ? accountList : []
		accountId.value =
			(accounts.value.find((account) => account.profile.id !== defaultAccount) ?? accounts.value[0])
				?.profile.id ?? ''
	} catch (err) {
		error.value = toError(err).message
	} finally {
		loading.value = false
	}
}

async function launch() {
	if (
		busy.value ||
		loading.value ||
		!accountId.value ||
		props.instance.quarantined ||
		props.instance.install_stage !== 'installed'
	)
		return
	busy.value = true
	error.value = ''
	try {
		const result = await runCopy(props.instance.id, accountId.value)
		await queryClient.invalidateQueries({ queryKey: instanceKeys.all })
		busy.value = false
		modal.value?.hide()
		await router.push(`/instance/${encodeURIComponent(result.instance_id)}/logs`)
	} catch (err) {
		error.value = toError(err).message
	} finally {
		busy.value = false
	}
}

defineExpose({ show })
</script>

<template>
	<NewModal
		ref="modal"
		:header="formatMessage(messages.title)"
		max-width="540px"
		:disable-close="busy || loading"
	>
		<div class="flex flex-col gap-4" :aria-busy="busy || loading">
			<p class="m-0">{{ formatMessage(messages.body) }}</p>
			<p v-if="playing" class="m-0 rounded-xl bg-surface-2 p-3 text-orange">
				{{ formatMessage(messages.saves) }}
			</p>
			<span v-if="loading">{{ formatMessage(commonMessages.loadingLabel) }}</span>
			<fieldset
				v-else-if="accounts.length"
				class="m-0 flex flex-col gap-3 rounded-xl border border-solid border-surface-5 p-3"
				:disabled="busy"
			>
				<legend class="font-semibold text-contrast">{{ formatMessage(messages.account) }}</legend>
				<label
					v-for="account in accounts"
					:key="account.profile.id"
					class="flex cursor-pointer items-center gap-3"
				>
					<input
						v-model="accountId"
						type="radio"
						name="copy-account"
						:value="account.profile.id"
						class="accent-brand"
					/>
					<span>{{ account.profile.name }}</span>
				</label>
			</fieldset>
			<p v-else class="m-0">{{ formatMessage(messages.empty) }}</p>
			<p v-if="error" role="alert" class="m-0 text-red">{{ error }}</p>
		</div>
		<template #actions>
			<div class="flex justify-end gap-2">
				<Button type="outlined" :disabled="busy || loading" @click="modal?.hide()">{{
					formatMessage(commonMessages.cancelButton)
				}}</Button>
				<Button
					type="colored"
					color="brand"
					:disabled="
						busy ||
						loading ||
						!accountId ||
						instance.quarantined ||
						instance.install_stage !== 'installed'
					"
					@click="launch"
					><SpinnerIcon v-if="busy" class="animate-spin" /><PlayIcon v-else />{{
						formatMessage(busy ? messages.busy : messages.launch)
					}}</Button
				>
			</div>
		</template>
	</NewModal>
</template>
