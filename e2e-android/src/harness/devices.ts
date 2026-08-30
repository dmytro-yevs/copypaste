export interface DeviceSectionContract {
  id: string;
  heading: string;
}

export const DEVICE_SECTION_CONTRACTS = [
  { id: "your-devices-heading", heading: "Your devices" },
  { id: "cloud-connection-heading", heading: "Cloud connection" },
  { id: "network-devices-heading", heading: "Discovered on your network" },
] as const satisfies readonly DeviceSectionContract[];

export interface DeviceSectionSnapshot {
  sectionCount: number;
  headingCount: number;
  headingText: string | null;
  rendered: boolean;
}

export function deviceSectionSatisfiesContract(
  snapshot: DeviceSectionSnapshot,
  contract: DeviceSectionContract,
): boolean {
  return (
    snapshot.sectionCount === 1 &&
    snapshot.headingCount === 1 &&
    snapshot.headingText === contract.heading &&
    snapshot.rendered
  );
}

export function allDeviceSectionsSatisfyContracts(
  snapshots: readonly DeviceSectionSnapshot[],
): boolean {
  return (
    snapshots.length === DEVICE_SECTION_CONTRACTS.length &&
    snapshots.every((snapshot, index) =>
      deviceSectionSatisfiesContract(snapshot, DEVICE_SECTION_CONTRACTS[index]!),
    )
  );
}
