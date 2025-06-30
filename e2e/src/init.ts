import {
    CONTRACTS,
    execShellCommand,
    get_signers,
    initContract,
    LCD, load_config,
    save_config,
    simulateAndBroadcast,
    storeCode,
    TestConfig
} from "./lib";
import {IdentifiedChannel} from "@terra-money/feather.js/dist/core/ibc/core";
import {createHash} from 'crypto'
import {Coin, Coins, MsgCreateDenom, MsgMint, MsgSend} from "@terra-money/feather.js";

const ORO_CODE_PATH = `${CONTRACTS}/cw20_oro.wasm`
const ORO_CONVERTER_PATH = `${CONTRACTS}/oro_token_converter.wasm`
const ORO_CONVERTER_NEUTRON_PATH = `${CONTRACTS}/oro_token_converter_neutron.wasm`
const CW20_ICS20_CODE_PATH = `${CONTRACTS}/cw20_ics20.wasm`

const {terra, neutron} = get_signers()

async function init_contracts(): Promise<TestConfig> {
    let code_id = await storeCode(LCD, terra.signer, "localterra-1", ORO_CODE_PATH)
    const cw20_init_msg = {
        name: "Oro Token",
        symbol: "ORO",
        decimals: 6,
        initial_balances: [
            {
                address: terra.address,
                amount: "1100000000000000",
            },
        ],
    }
    const oro_token = await initContract(LCD, terra.signer, "localterra-1", code_id, cw20_init_msg)

    code_id = await storeCode(LCD, terra.signer, "localterra-1", CW20_ICS20_CODE_PATH)
    const cw20_ics20_init_msg = {
        default_timeout: 300,
        gov_contract: terra.address,
        allowlist: [
            {contract: oro_token},
        ]
    }
    const cw20_ics20 = await initContract(LCD, terra.signer, "localterra-1", code_id, cw20_ics20_init_msg)

    return {
        oro_token: oro_token,
        cw20_ics20: cw20_ics20,
    }
}

async function setup_channel(a_port: string, b_port: string) {
    const hermes_cmd = `docker exec hermes hermes create channel --a-chain localterra-1 --new-client-connection --b-chain localneutron-1 --a-port ${a_port} --b-port ${b_port} --chan-version ics20-1 --yes`
    console.log(hermes_cmd)
    await execShellCommand(hermes_cmd)
    const queryCb = ({channels,}: Record<string, any>) => {
        let chans = (channels as IdentifiedChannel[])
            .sort((a, b) => {
                const [a_chan_num, b_chan_num] = [
                    parseInt(a.channel_id.match(/\d+$/g)![0]),
                    parseInt(b.channel_id.match(/\d+$/g)![0]),
                ]
                if (a_chan_num < b_chan_num) {
                    return 1
                } else if (a_chan_num > b_chan_num) {
                    return -1
                } else {
                    return 0
                }
            })
        return chans[0].channel_id
    }

    const terra_channel = await LCD.ibc.channels("localterra-1")
        .then(queryCb)
    const neutron_channel = await LCD.ibc.channels("localneutron-1")
        .then(queryCb)

    return [terra_channel, neutron_channel]
}

const init_tf_oro = async (config: TestConfig) => {
    const create_msg = new MsgCreateDenom(
        neutron.address,
        "uoro"
    )
    const token_denom = `factory/${neutron.address}/uoro`
    const mint_msg = new MsgMint(
        neutron.address,
        new Coin(token_denom, 1_100_000_000_000000)
    )
    await simulateAndBroadcast(LCD, neutron.signer, neutron.chain_id, [create_msg, mint_msg])

    // Setup transfer <> transfer IBC channel
    const [terra_channel, neutron_channel] = await setup_channel("transfer", "transfer")
    config.new_terra_channel = terra_channel
    config.new_neutron_channel = neutron_channel

    config.oro_tf_denom = token_denom
    config.oro_tf_ibc_denom = determine_ibc_denom(config.new_terra_channel!, token_denom)

    console.log(`New ORO IBC denom on Terra for path transfer/${config.new_neutron_channel}/${token_denom}\n${config.oro_tf_ibc_denom}`)

    return config
}

const init_oro_converters = async (config: TestConfig) => {
    const terra_converter_code_id = await storeCode(LCD, terra.signer, terra.chain_id, ORO_CONVERTER_PATH)
    const terra_converter_init_msg = {
        old_oro_asset_info: {token: {contract_addr: config.oro_token}},
        new_oro_denom: config.oro_tf_ibc_denom!,
    }
    config.terra_converter = await initContract(LCD, terra.signer, terra.chain_id, terra_converter_code_id, terra_converter_init_msg)

    const neutron_converter_code_id = await storeCode(LCD, neutron.signer, neutron.chain_id, ORO_CONVERTER_NEUTRON_PATH)
    const neutron_converter_init_msg = {
        old_oro_asset_info: {native_token: {denom: config.oro_ibc_denom}},
        new_oro_denom: config.oro_tf_denom!,
        outpost_burn_params: {
            terra_burn_addr: config.terra_converter,
            old_oro_transfer_channel: config.neutron_channel
        }
    }
    config.neutron_converter = await initContract(LCD, neutron.signer, neutron.chain_id, neutron_converter_code_id, neutron_converter_init_msg)

    return config
}

const determine_ibc_denom = (channel: string, orig_denom: string) => {
    return "ibc/" + createHash('sha256')
        .update(`transfer/${channel}/${orig_denom}`)
        .digest("hex")
        .toUpperCase()
}

const init = async function () {
    let config = await init_contracts()
    const [terra_channel, neutron_channel] = await setup_channel(`wasm.${config.cw20_ics20}`, "transfer")

    const denom_hash = determine_ibc_denom(neutron_channel, `cw20:${config.oro_token}`)

    console.log(`ORO denom for path transfer/${neutron_channel}/cw20:${config.oro_token}\n${denom_hash}`)

    config = {
        ...config,
        terra_channel: terra_channel,
        neutron_channel: neutron_channel,
        oro_ibc_denom: denom_hash
    }
    save_config(config)

    config = await init_tf_oro(config)
    save_config(config)

    config = await init_oro_converters(config)
    save_config(config)
}

init().catch(console.error);