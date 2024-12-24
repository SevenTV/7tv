<script lang="ts">
	import { graphql } from "$/gql";
	import { gqlClient } from "$/lib/gql";
	import { t } from "svelte-i18n";
	import { refreshUser, user } from "$/lib/auth";
	import PaintComponent from "$/components/paint.svelte";
	import BadgeComponent from "$/components/badge.svelte";
	import Spinner from "$/components/spinner.svelte";
	import type { Badge, Paint } from "$/gql/graphql";
	import type { PageData } from "./$types";
	import Select, { type Option } from "$/components/input/select.svelte";
	import TextInput from "$/components/input/text-input.svelte";
	import { Empty, MagnifyingGlass } from "phosphor-svelte";
	import LayoutButtons from "$/components/emotes/layout-buttons.svelte";
	import HideOn from "$/components/hide-on.svelte";
	import type { Layout } from "$/lib/layout";
	import Radio from "$/components/input/radio.svelte";
	import { setActiveBadge, setActivePaint } from "$/lib/userMutations";

	let { data }: { data: PageData } = $props();

	async function queryInventory(id: string) {
		const res = await gqlClient()
			.query(
				graphql(`
					query UserInventory($id: Id!) {
						users {
							user(id: $id) {
								inventory {
									badges {
										to {
											badge {
												id
												name
												description
												images {
													url
													mime
													size
													scale
													width
													height
													frameCount
												}
											}
										}
									}
									paints {
										from {
											__typename
											... on EntitlementNodeRole {
												role {
													id
													name
												}
											}
											... on EntitlementNodeSubscriptionBenefit {
												subscriptionBenefit {
													id
													name
												}
											}
											... on EntitlementNodeSpecialEvent {
												specialEvent {
													id
													name
												}
											}
										}
										to {
											paint {
												id
												name
												data {
													layers {
														id
														ty {
															__typename
															... on PaintLayerTypeSingleColor {
																color {
																	hex
																}
															}
															... on PaintLayerTypeLinearGradient {
																angle
																repeating
																stops {
																	at
																	color {
																		hex
																	}
																}
															}
															... on PaintLayerTypeRadialGradient {
																repeating
																stops {
																	at
																	color {
																		hex
																	}
																}
																shape
															}
															... on PaintLayerTypeImage {
																images {
																	url
																	mime
																	size
																	scale
																	width
																	height
																	frameCount
																}
															}
														}
														opacity
													}
													shadows {
														color {
															hex
														}
														offsetX
														offsetY
														blur
													}
												}
											}
										}
									}
								}
							}
						}
					}
				`),
				{ id },
			)
			.toPromise();

		if (res.error || !res.data) {
			throw res.error;
		}

		const inventory = res.data.users.user?.inventory;

		if (!res.data.users.user || !inventory) {
			return undefined;
		}

		const badges = inventory.badges
			.filter((b) => b.to.badge)
			.reduce(
				(map, b) => {
					map[b.to.badge!.id] = b.to.badge as Badge;
					return map;
				},
				{} as { [key: string]: Badge },
			);

		const paints: { [key: string]: { paint: Paint; sourceKey?: string; sourceName?: string } } = {};
		const paintFilters: Option[] = [];

		for (const entitlement of inventory.paints.filter((p) => p.to.paint)) {
			if (entitlement.from.__typename === "EntitlementNodeRole" && entitlement.from.role) {
				const roleId = entitlement.from.role.id;
				const roleName = entitlement.from.role.name;

				paints[entitlement.to.paint!.id] = {
					paint: entitlement.to.paint as Paint,
					sourceKey: roleId,
					sourceName: roleName,
				};

				if (!paintFilters.some((f) => f.value === roleId)) {
					paintFilters.push({
						label: roleName,
						value: roleId,
					});
				}
			} else if (
				entitlement.from.__typename === "EntitlementNodeSubscriptionBenefit" &&
				entitlement.from.subscriptionBenefit
			) {
				const benefitId = entitlement.from.subscriptionBenefit.id;
				const benefitName = entitlement.from.subscriptionBenefit.name;

				paints[entitlement.to.paint!.id] = {
					paint: entitlement.to.paint as Paint,
					sourceKey: benefitId,
					sourceName: benefitName,
				};

				if (!paintFilters.some((f) => f.value === benefitId)) {
					paintFilters.push({
						label: benefitName,
						value: benefitId,
					});
				}
			} else if (
				entitlement.from.__typename === "EntitlementNodeSpecialEvent" &&
				entitlement.from.specialEvent
			) {
				const eventId = entitlement.from.specialEvent.id;
				const eventName = entitlement.from.specialEvent.name;

				paints[entitlement.to.paint!.id] = {
					paint: entitlement.to.paint as Paint,
					sourceKey: eventId,
					sourceName: eventName,
				};

				if (!paintFilters.some((f) => f.value === eventId)) {
					paintFilters.push({
						label: eventName,
						value: eventId,
					});
				}
			} else {
				paints[entitlement.to.paint!.id] = {
					paint: entitlement.to.paint as Paint,
				};
			}
		}

		paintFilters.sort((a, b) => a.value.localeCompare(b.value));

		return {
			badges,
			paints,
			paintFilters,
		};
	}

	let inventory = $derived(queryInventory(data.id));

	let editingEnabled = $derived($user?.id === data.id || $user?.permissions.user.manageAny);

	let paintQuery = $state("");
	let paintFilter = $state<string>("");
	let paintsLayout = $state<Layout>("big-grid");

	let badgeQuery = $state("");
	let badgesLayout = $state<Layout>("big-grid");

	let originalBadgeId: string | null | undefined;
	let originalPaintId: string | null | undefined;
	let activeBadge = $state<string>();
	let activePaint = $state<string>();

	$effect(() => {
		data.streamed.userRequest.value.then((data) => {
			if (!data) {
				return;
			}

			originalBadgeId = data.style.activeBadgeId;
			originalPaintId = data.style.activePaintId;

			activeBadge = data.style.activeBadgeId === null ? "none" : data.style.activeBadgeId;
			activePaint = data.style.activePaintId === null ? "none" : data.style.activePaintId;
		});
	});

	function selectValueToRealValue(value: string) {
		return value === "none" ? null : value;
	}

	let badgeLoading = $state(false);

	$effect(() => {
		if (activeBadge === undefined) {
			return;
		}

		const activeBadgeValue = selectValueToRealValue(activeBadge);

		if (originalBadgeId !== activeBadgeValue) {
			badgeLoading = true;

			const promise = setActiveBadge(data.id, activeBadgeValue);
			data.streamed.userRequest.value = promise;
			promise
				.then(() => {
					refreshUser();
				})
				.finally(() => {
					badgeLoading = false;
				});
		}
	});

	let paintLoading = $state(false);

	$effect(() => {
		if (activePaint === undefined) {
			return;
		}

		const activePaintValue = selectValueToRealValue(activePaint);

		if (originalPaintId !== activePaintValue) {
			paintLoading = true;

			const promise = setActivePaint(data.id, activePaintValue);
			data.streamed.userRequest.value = promise;
			promise
				.then(() => {
					refreshUser();
				})
				.finally(() => {
					paintLoading = false;
				});
		}
	});

	let paintMouseOver = $state();
</script>

<svelte:head>
	<title>Cosmetics - {$t("page_titles.suffix")}</title>
</svelte:head>

{#await inventory}
	<div class="spinner-container">
		<Spinner />
	</div>
{:then inventory}
	{#if inventory}
		{@const badgeIds = new Set(
			Object.values(inventory.badges)
				.filter((b) => !badgeQuery || b.name.toLowerCase().includes(badgeQuery))
				.map((b) => b.id),
		)}
		{@const paintIds = new Set(
			Object.values(inventory.paints)
				.filter(
					(p) =>
						(!paintFilter || p.sourceKey === paintFilter) &&
						(!paintQuery || p.paint.name.toLowerCase().includes(paintQuery.trim().toLowerCase())),
				)
				.map((p) => p.paint.id),
		)}
		<div class="layout">
			<div class="header-container">
				<h2>Cosmetics</h2>
			</div>
			<section>
				<div class="header">
					<h1>
						Badges
						{#if badgeLoading}
							<Spinner />
						{/if}
					</h1>
					<div class="buttons">
						<HideOn mobile>
							<TextInput placeholder="Search" bind:value={badgeQuery}>
								{#snippet icon()}
									<MagnifyingGlass />
								{/snippet}
							</TextInput>
						</HideOn>
						<LayoutButtons bind:value={badgesLayout} allowedLayouts={["big-grid", "list"]} />
					</div>
				</div>
				<div
					class="cosmetics"
					class:grid={badgesLayout === "big-grid"}
					class:list={badgesLayout === "list"}
				>
					<Radio
						option
						name="badge"
						value="none"
						bind:group={activeBadge}
						disabled={!editingEnabled || badgeLoading}
						style="padding-block: 0.75rem; justify-content: start; overflow: hidden;"
					>
						<Empty />
						None
					</Radio>
					{#each badgeIds as badgeId}
						{@const badge = inventory.badges[badgeId]}
						<Radio
							option
							name="badge"
							value={badge.id}
							bind:group={activeBadge}
							disabled={!editingEnabled || badgeLoading}
							style="padding-block: 0.75rem; justify-content: start; overflow: hidden;"
						>
							<BadgeComponent {badge} size={2 * 16} enableDialog={activeBadge === badge.id} />
							<span class="name">{badge.name}</span>
							{#if badgesLayout === "list"}
								<span class="description">{badge.description}</span>
							{/if}
						</Radio>
					{/each}
				</div>
			</section>
			<section>
				<div class="header">
					<h1>
						Paints
						{#if paintLoading}
							<Spinner />
						{/if}
					</h1>
					<div class="buttons">
						{#if inventory.paintFilters.length > 0}
							<Select
								bind:selected={paintFilter}
								options={[{ label: "None", value: "" }, ...inventory.paintFilters]}
							/>
						{/if}
						<HideOn mobile>
							<TextInput placeholder="Search" bind:value={paintQuery}>
								{#snippet icon()}
									<MagnifyingGlass />
								{/snippet}
							</TextInput>
						</HideOn>
						<LayoutButtons bind:value={paintsLayout} allowedLayouts={["big-grid", "list"]} />
					</div>
				</div>
				<div
					class="cosmetics"
					class:grid={paintsLayout === "big-grid"}
					class:list={paintsLayout === "list"}
				>
					<Radio
						option
						name="paint"
						value="none"
						bind:group={activePaint}
						disabled={!editingEnabled || paintLoading}
						style="padding-block: 0.75rem; justify-content: start; overflow: hidden;"
					>
						<Empty />
						None
					</Radio>
					{#each paintIds as paintId}
						{@const paint = inventory.paints[paintId]}
						{@const paintName = paint.paint.name.length > 0 ? paint.paint.name : paint.paint.id}
						<Radio
							option
							name="paint"
							value={paint.paint.id}
							bind:group={activePaint}
							disabled={!editingEnabled || paintLoading}
							style="padding-block: 0.75rem; justify-content: start; overflow: hidden;"
							onmouseover={() => (paintMouseOver = paint.paint.id)}
							onmouseleave={() => (paintMouseOver = undefined)}
						>
							<PaintComponent
								paint={paint.paint}
								style="font-size: 0.875rem; font-weight: 500;"
								enableDialog={!editingEnabled || paint.paint.id === activePaint}
							>
								{#if paintMouseOver === paint.paint.id}
									{#await data.streamed.userRequest.value}
										{paintName}
									{:then data}
										{data.mainConnection?.platformDisplayName ?? paintName}
									{/await}
								{:else}
									{paintName}
								{/if}
							</PaintComponent>
							{#if paintsLayout === "list" && paint.sourceName}
								<span class="description">{paint.sourceName}</span>
							{/if}
						</Radio>
					{/each}
				</div>
			</section>
		</div>
	{/if}
{/await}

<style lang="scss">
	.header-container {
		display: flex;
		justify-content: space-between;
		height: 40px;
		
		h2 {
			font-family: "AKONY";
			font-size: 1.5rem;
			font-weight: 700;
			margin: auto 0;
		}
	}

	.spinner-container {
		display: flex;
		justify-content: center;
		align-items: center;
		height: 100%;
	}

	.layout {
		overflow: auto;
		scrollbar-gutter: stable;
		min-height: 100%;

		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	section {
		display: flex;
		flex-direction: column;
		gap: 1rem;

		padding: 1rem;
		background-color: var(--bg-medium);
		border-radius: 0.5rem;
	}

	h1 {
		font-size: 1rem;
		font-weight: 500;
	}

	.header {
		display: flex;
		justify-content: space-between;
		align-items: center;
	}

	.buttons {
		display: flex;
		gap: 0.5rem;
		align-items: center;
	}

	.cosmetics {
		gap: 0.5rem;

		&.grid {
			display: grid;
			grid-template-columns: repeat(auto-fill, minmax(14rem, 1fr));
		}

		&.list {
			display: flex;
			flex-direction: column;
		}
	}

	.name {
		font-size: 0.75rem;
		font-weight: 500;
	}

	.description {
		margin-left: auto;

		color: var(--text-light);
		font-size: 0.75rem;
	}
</style>
