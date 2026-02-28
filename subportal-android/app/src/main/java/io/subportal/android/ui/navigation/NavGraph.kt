package io.subportal.android.ui.navigation

import androidx.compose.runtime.Composable
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import io.subportal.android.ui.screens.EnrollScreen
import io.subportal.android.ui.screens.ServerListScreen

object Routes {
    const val SERVER_LIST = "server_list"
    const val ENROLL = "enroll"
}

@Composable
fun SubportalNavGraph(navController: NavHostController) {
    NavHost(navController = navController, startDestination = Routes.SERVER_LIST) {
        composable(Routes.SERVER_LIST) {
            ServerListScreen(
                onEnrollClick = { navController.navigate(Routes.ENROLL) }
            )
        }
        composable(Routes.ENROLL) {
            EnrollScreen(
                onEnrolled = { navController.popBackStack() },
                onBack = { navController.popBackStack() }
            )
        }
    }
}
