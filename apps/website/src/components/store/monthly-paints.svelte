<script lang="ts">
	import StoreSection from "./store-section.svelte";
	import { DotsThreeVertical, PaintBrush, Repeat } from "phosphor-svelte";
	import { t } from "svelte-i18n";
	import moment from "moment/min/moment-with-locales";
	import Button from "../input/button.svelte";
	import DropDown from "../drop-down.svelte";
	import type { Paint } from "$/gql/graphql";
	import PaintComponent from "../paint.svelte";
	import { user } from "$/lib/auth";

	let { paints }: { paints: Paint[] } = $props();

	let renewMoment = moment().utc().add(1, "month").startOf("month");
</script>

<StoreSection title={$t("pages.store.subscription.monthly_paints")}>
	{#snippet header()}
		<div class="buttons">
			<div class="renew-countdown">
				<Repeat />
				<span title={renewMoment.format("lll")}>{renewMoment.fromNow(true)}</span>
			</div>
			{#if $user}
				<DropDown>
					{#snippet dropdown()}
						<Button big href="/users/{$user.id}/cosmetics">
							{#snippet icon()}
								<PaintBrush />
							{/snippet}
							Your Paints
						</Button>
					{/snippet}
					<Button secondary>
						{#snippet icon()}
							<DotsThreeVertical />
						{/snippet}
					</Button>
				</DropDown>
			{/if}
		</div>
	{/snippet}
	{#each paints as paint}
		<div class="paint">
			<PaintComponent {paint} style="font-weight: 700" enableDialog>
				{paint.name}
			</PaintComponent>
		</div>
	{/each}
</StoreSection>

<style lang="scss">
	.buttons {
		display: flex;
		align-items: center;
		gap: 0.75rem;
	}

	.renew-countdown {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		white-space: nowrap;

		color: var(--text-light);
		font-size: 0.75rem;
	}

	.paint {
		padding: 0.5rem 1rem;
		border-radius: 0.5rem;
		background-color: var(--bg-light);
	}
</style>
